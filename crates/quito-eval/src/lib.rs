//! Tree-walk evaluator: AST -> CSG tree ([`quito_ir::Node`]).
//!
//! Scoping matches OpenSCAD: ordinary variables, functions, and modules are
//! lexically scoped (closures capture the scope chain at definition time),
//! while `$` special variables are dynamically scoped (a separate frame stack
//! that mirrors execution nesting). Function values and module `children()`
//! both close over their definition / call-site environments.

mod value;

pub use value::{format_number, Value};

use quito_ir::{FragmentSpec, Node, Vec3};
use quito_syntax::ast::*;
use std::collections::{HashMap, HashSet};

/// A file loaded by an `include`/`use` resolver.
pub struct LoadedFile {
    /// A canonical key identifying the file (for cycle detection).
    pub key: String,
    pub source: String,
    /// Directory of the loaded file (base for its own relative includes).
    pub dir: String,
}

/// Resolves `include`/`use` paths to source. Native builds read from disk;
/// the browser can supply an in-memory map.
pub trait FileResolver {
    fn load(&self, path: &str, from_dir: &str) -> Option<LoadedFile>;
}

/// A resolver that never finds anything (include/use become warnings).
pub struct NullResolver;
impl FileResolver for NullResolver {
    fn load(&self, _path: &str, _from_dir: &str) -> Option<LoadedFile> {
        None
    }
}

const MAX_RANGE_ITERS: usize = 10_000_000;
const MAX_CALL_DEPTH: usize = 20_000;

#[derive(Debug, Clone, thiserror::Error)]
#[error("{0}")]
pub struct EvalError(pub String);

type EResult<T> = Result<T, EvalError>;

fn err<T>(msg: impl Into<String>) -> EResult<T> {
    Err(EvalError(msg.into()))
}

use std::cell::RefCell;
use std::rc::Rc;

type ScopeRef = Rc<RefCell<Scope>>;

/// A function value: parameters, body, and the lexical environment (scope
/// chain) captured at definition time.
pub struct FnClosure {
    params: Vec<Param>,
    body: Expr,
    env: Vec<ScopeRef>,
}

/// Outcome of evaluating a function body in tail position.
enum TailResult {
    Value(Value),
    /// Re-invoke the same function with these freshly-bound arguments.
    TailCall(HashMap<String, Value>),
}

/// A module definition with its captured lexical environment.
struct ModClosure {
    params: Vec<Param>,
    body: Vec<Stmt>,
    env: Vec<ScopeRef>,
}

#[derive(Default)]
struct Scope {
    vars: HashMap<String, Value>,
    funcs: HashMap<String, Rc<FnClosure>>,
    modules: HashMap<String, Rc<ModClosure>>,
}

/// The output of evaluating a program.
pub struct EvalOutput {
    pub node: Node,
    pub echoes: Vec<String>,
    pub warnings: Vec<String>,
}

struct Interp<'a> {
    /// Lexical scope chain (swapped to a closure's captured env during calls).
    scopes: Vec<ScopeRef>,
    /// Dynamic frames for `$` variables (mirrors execution nesting; NOT swapped
    /// on calls, giving `$vars` dynamic scoping).
    specials: Vec<HashMap<String, Value>>,
    echoes: Vec<String>,
    warnings: Vec<String>,
    root: Option<Node>,
    depth: usize,
    /// For each active module call: the child statements plus the caller's
    /// lexical scope chain, so `children()` evaluates them in the call site.
    children_stack: Vec<(Vec<Stmt>, Vec<ScopeRef>)>,
    /// `include`/`use` file resolver.
    resolver: &'a dyn FileResolver,
    /// Directory of the file currently being evaluated (for relative includes).
    cur_dir: String,
    /// Files currently being loaded, for include/use cycle detection.
    loading: HashSet<String>,
}

/// Evaluate a parsed program into a CSG tree plus console output (no file
/// access; `include`/`use` become warnings).
pub fn eval_program(prog: &Program) -> EResult<EvalOutput> {
    eval_program_with(prog, &NullResolver, ".")
}

/// Evaluate a program with `include`/`use` support via `resolver`, resolving
/// relative paths against `base_dir`.
pub fn eval_program_with(
    prog: &Program,
    resolver: &dyn FileResolver,
    base_dir: &str,
) -> EResult<EvalOutput> {
    let mut base = Scope::default();
    base.vars.insert("PI".into(), Value::Number(std::f64::consts::PI));

    // `$` special variables live in the dynamic frame stack.
    let mut globals = HashMap::new();
    globals.insert("$fn".to_string(), Value::Number(0.0));
    globals.insert("$fa".to_string(), Value::Number(12.0));
    globals.insert("$fs".to_string(), Value::Number(2.0));
    globals.insert("$t".to_string(), Value::Number(0.0));
    globals.insert("$preview".to_string(), Value::Bool(true));

    let mut interp = Interp {
        scopes: vec![Rc::new(RefCell::new(base))],
        specials: vec![globals],
        echoes: Vec::new(),
        warnings: Vec::new(),
        root: None,
        depth: 0,
        children_stack: Vec::new(),
        resolver,
        cur_dir: base_dir.to_string(),
        loading: HashSet::new(),
    };

    let nodes = interp.eval_stmts(prog)?;
    let node = interp.root.take().unwrap_or_else(|| Node::group(nodes));
    Ok(EvalOutput {
        node,
        echoes: interp.echoes,
        warnings: interp.warnings,
    })
}

impl Interp<'_> {
    // ---- scope helpers -------------------------------------------------

    fn push_scope(&mut self) {
        self.scopes.push(Rc::new(RefCell::new(Scope::default())));
        self.specials.push(HashMap::new());
    }

    fn pop_scope(&mut self) {
        self.scopes.pop();
        self.specials.pop();
    }

    fn set_var(&mut self, name: &str, val: Value) {
        if name.starts_with('$') {
            self.specials.last_mut().unwrap().insert(name.to_string(), val);
        } else {
            self.scopes
                .last()
                .unwrap()
                .borrow_mut()
                .vars
                .insert(name.to_string(), val);
        }
    }

    fn lookup_var(&self, name: &str) -> Value {
        if name.starts_with('$') {
            for frame in self.specials.iter().rev() {
                if let Some(v) = frame.get(name) {
                    return v.clone();
                }
            }
        } else {
            for scope in self.scopes.iter().rev() {
                if let Some(v) = scope.borrow().vars.get(name) {
                    return v.clone();
                }
            }
        }
        Value::Undef
    }

    fn lookup_func(&self, name: &str) -> Option<Rc<FnClosure>> {
        for scope in self.scopes.iter().rev() {
            if let Some(f) = scope.borrow().funcs.get(name) {
                return Some(f.clone());
            }
        }
        None
    }

    fn lookup_module(&self, name: &str) -> Option<Rc<ModClosure>> {
        for scope in self.scopes.iter().rev() {
            if let Some(m) = scope.borrow().modules.get(name) {
                return Some(m.clone());
            }
        }
        None
    }

    // ---- statements ----------------------------------------------------

    fn eval_stmts(&mut self, stmts: &[Stmt]) -> EResult<Vec<Node>> {
        // Splice `include`d files in first (only when present, to avoid cloning).
        let expanded;
        let effective: &[Stmt] = if stmts.iter().any(|s| matches!(s, Stmt::Include { .. })) {
            expanded = self.expand_includes(stmts)?;
            &expanded
        } else {
            stmts
        };

        // Phases 1 (definitions + `use` imports) and 2 (assignments).
        self.eval_defs_and_assigns(effective)?;

        // Phase 3: geometry.
        let mut out = Vec::new();
        for s in effective {
            match s {
                Stmt::Assign { .. }
                | Stmt::FunctionDef { .. }
                | Stmt::ModuleDef { .. }
                | Stmt::Use { .. }
                | Stmt::Include { .. } => {}
                _ => out.extend(self.eval_geom(s)?),
            }
        }
        Ok(out)
    }

    /// Phase 1 (hoist definitions + process `use` imports) and phase 2 (hoist
    /// assignments, last write wins) into the current scope.
    fn eval_defs_and_assigns(&mut self, stmts: &[Stmt]) -> EResult<()> {
        for s in stmts {
            match s {
                Stmt::FunctionDef { name, params, body } => {
                    let env = self.scopes.clone();
                    self.scopes.last().unwrap().borrow_mut().funcs.insert(
                        name.clone(),
                        Rc::new(FnClosure {
                            params: params.clone(),
                            body: body.clone(),
                            env,
                        }),
                    );
                }
                Stmt::ModuleDef { name, params, body } => {
                    let env = self.scopes.clone();
                    self.scopes.last().unwrap().borrow_mut().modules.insert(
                        name.clone(),
                        Rc::new(ModClosure {
                            params: params.clone(),
                            body: body.clone(),
                            env,
                        }),
                    );
                }
                Stmt::Use { path } => self.import_use(path)?,
                _ => {}
            }
        }
        for s in stmts {
            if let Stmt::Assign { name, value } = s {
                let v = self.eval_expr(value)?;
                self.set_var(name, v);
            }
        }
        Ok(())
    }

    /// Recursively splice `include`d files' top-level statements in place.
    fn expand_includes(&mut self, stmts: &[Stmt]) -> EResult<Vec<Stmt>> {
        let mut out = Vec::new();
        for s in stmts {
            match s {
                Stmt::Include { path } => {
                    let Some(lf) = self.resolver.load(path, &self.cur_dir) else {
                        self.warnings.push(format!("Can't open include file '{path}'"));
                        continue;
                    };
                    if !self.loading.insert(lf.key.clone()) {
                        continue; // cycle: already loading this file
                    }
                    let prog = quito_syntax::parse(&lf.source).map_err(|e| {
                        EvalError(format!("in include '{path}': {}", e.message))
                    })?;
                    let prev = std::mem::replace(&mut self.cur_dir, lf.dir.clone());
                    let expanded = self.expand_includes(&prog);
                    self.cur_dir = prev;
                    self.loading.remove(&lf.key);
                    out.extend(expanded?);
                }
                other => out.push(other.clone()),
            }
        }
        Ok(out)
    }

    /// Import a `use`d file's module/function definitions (only) into the
    /// current scope. The file is evaluated in isolation; its definitions close
    /// over its own top-level scope so they can use its helpers/constants.
    fn import_use(&mut self, path: &str) -> EResult<()> {
        let Some(lf) = self.resolver.load(path, &self.cur_dir) else {
            self.warnings.push(format!("Can't open 'use' file '{path}'"));
            return Ok(());
        };
        if !self.loading.insert(lf.key.clone()) {
            return Ok(()); // cycle
        }
        let prog = quito_syntax::parse(&lf.source)
            .map_err(|e| EvalError(format!("in use '{path}': {}", e.message)))?;

        let file_scope: ScopeRef = Rc::new(RefCell::new(Scope::default()));
        let base = self.scopes[0].clone();
        let saved = std::mem::replace(&mut self.scopes, vec![base, file_scope.clone()]);
        let prev_dir = std::mem::replace(&mut self.cur_dir, lf.dir.clone());
        self.specials.push(HashMap::new());

        let expanded = self.expand_includes(&prog);
        let result = expanded.and_then(|eff| self.eval_defs_and_assigns(&eff));

        self.specials.pop();
        self.cur_dir = prev_dir;
        self.scopes = saved;
        self.loading.remove(&lf.key);
        result?;

        // Import only the definitions.
        let fs = file_scope.borrow();
        let target = self.scopes.last().unwrap();
        let mut t = target.borrow_mut();
        for (k, v) in fs.funcs.iter() {
            t.funcs.insert(k.clone(), v.clone());
        }
        for (k, v) in fs.modules.iter() {
            t.modules.insert(k.clone(), v.clone());
        }
        Ok(())
    }

    fn eval_geom(&mut self, stmt: &Stmt) -> EResult<Vec<Node>> {
        match stmt {
            Stmt::Block(stmts) => {
                self.push_scope();
                let r = self.eval_stmts(stmts);
                self.pop_scope();
                r
            }
            Stmt::If { cond, then, els } => {
                let c = self.eval_expr(cond)?;
                let branch = if c.truthy() { then } else { els };
                self.push_scope();
                let r = self.eval_stmts(branch);
                self.pop_scope();
                r
            }
            Stmt::For { bindings, body } => self.eval_for(bindings, body),
            Stmt::Let { bindings, body } => {
                self.push_scope();
                for (n, e) in bindings {
                    let v = self.eval_expr(e)?;
                    self.set_var(n, v);
                }
                let r = self.eval_stmts(body);
                self.pop_scope();
                r
            }
            Stmt::ModuleCall {
                modifier,
                name,
                args,
                children,
            } => self.eval_module_call(*modifier, name, args, children),
            _ => Ok(Vec::new()),
        }
    }

    fn eval_for(&mut self, bindings: &[(String, Expr)], body: &[Stmt]) -> EResult<Vec<Node>> {
        let mut out = Vec::new();
        self.eval_for_rec(bindings, body, &mut out)?;
        Ok(out)
    }

    fn eval_for_rec(
        &mut self,
        bindings: &[(String, Expr)],
        body: &[Stmt],
        out: &mut Vec<Node>,
    ) -> EResult<()> {
        if bindings.is_empty() {
            self.push_scope();
            let r = self.eval_stmts(body);
            self.pop_scope();
            out.extend(r?);
            return Ok(());
        }
        let (name, expr) = &bindings[0];
        let iter = self.eval_expr(expr)?;
        let values = iter_values(&iter)?;
        for v in values {
            self.push_scope();
            self.set_var(name, v);
            let r = self.eval_for_rec(&bindings[1..], body, out);
            self.pop_scope();
            r?;
        }
        Ok(())
    }

    fn eval_module_call(
        &mut self,
        modifier: Option<Modifier>,
        name: &str,
        args: &[Arg],
        children: &[Stmt],
    ) -> EResult<Vec<Node>> {
        if modifier == Some(Modifier::Disable) {
            return Ok(Vec::new());
        }

        let node = self.dispatch_module(name, args, children)?;

        if modifier == Some(Modifier::Root) {
            self.root = Some(node.clone());
        }
        // `#` highlight and `%` background are visual-only; passed through in M0.
        if matches!(node, Node::Empty) {
            Ok(Vec::new())
        } else {
            Ok(vec![node])
        }
    }

    fn dispatch_module(&mut self, name: &str, args: &[Arg], children: &[Stmt]) -> EResult<Node> {
        match name {
            "cube" => self.b_cube(args),
            "sphere" => self.b_sphere(args),
            "cylinder" => self.b_cylinder(args),
            "polyhedron" => self.b_polyhedron(args),
            "translate" => self.transform(args, children, TransformKind::Translate),
            "rotate" => self.transform(args, children, TransformKind::Rotate),
            "scale" => self.transform(args, children, TransformKind::Scale),
            "union" => Ok(Node::Union(self.eval_children(children)?)),
            "difference" => Ok(Node::Difference(self.eval_children(children)?)),
            "intersection" => Ok(Node::Intersection(self.eval_children(children)?)),
            "group" => Ok(Node::group(self.eval_children(children)?)),
            "echo" => self.b_echo(args, children),
            "assert" => self.b_assert(args, children),
            "children" => self.b_children(args),
            _ => {
                if let Some(def) = self.lookup_module(name) {
                    self.instantiate_module(&def, args, children)
                } else {
                    self.warnings
                        .push(format!("Ignoring unknown module '{name}'"));
                    Ok(Node::Empty)
                }
            }
        }
    }

    fn eval_children(&mut self, children: &[Stmt]) -> EResult<Vec<Node>> {
        self.push_scope();
        let r = self.eval_stmts(children);
        self.pop_scope();
        r
    }

    fn instantiate_module(
        &mut self,
        def: &Rc<ModClosure>,
        args: &[Arg],
        children: &[Stmt],
    ) -> EResult<Node> {
        // Arguments are evaluated in the caller's scope; the body runs in the
        // module's captured (lexical) environment.
        let bound = self.bind_params(&def.params, args)?;
        let caller_scopes = self.scopes.clone();
        let saved = std::mem::replace(&mut self.scopes, def.env.clone());
        self.push_scope();
        for (k, v) in bound {
            self.set_var(&k, v);
        }
        self.set_var("$children", Value::Number(children.len() as f64));
        self.children_stack.push((children.to_vec(), caller_scopes));
        let r = self.eval_stmts(&def.body);
        self.children_stack.pop();
        self.pop_scope();
        self.scopes = saved;
        Ok(Node::group(r?))
    }

    /// `children()` / `children(i)` / `children([indices|range])`. Children are
    /// evaluated in the caller's lexical scope (where they were written).
    fn b_children(&mut self, args: &[Arg]) -> EResult<Node> {
        let Some((kids, caller_scopes)) = self.children_stack.last().cloned() else {
            return Ok(Node::Empty);
        };
        // The index selector is evaluated in the module's scope.
        let idxs: Option<Vec<usize>> = if args.is_empty() {
            None
        } else {
            let sel = self.first_positional(args)?;
            Some(match sel {
                Value::Number(n) => vec![n as usize],
                Value::Vector(ref v) => {
                    v.iter().filter_map(Value::as_number).map(|n| n as usize).collect()
                }
                Value::Range { .. } => iter_values(&sel)?
                    .iter()
                    .filter_map(Value::as_number)
                    .map(|n| n as usize)
                    .collect(),
                _ => Vec::new(),
            })
        };
        // Evaluate the child geometry in the caller's lexical environment.
        let saved = std::mem::replace(&mut self.scopes, caller_scopes);
        self.push_scope();
        let result = (|| -> EResult<Node> {
            match idxs {
                None => Ok(Node::group(self.eval_stmts(&kids)?)),
                Some(idxs) => {
                    let mut out = Vec::new();
                    for i in idxs {
                        if let Some(stmt) = kids.get(i) {
                            out.extend(self.eval_geom(stmt)?);
                        }
                    }
                    Ok(Node::group(out))
                }
            }
        })();
        self.pop_scope();
        self.scopes = saved;
        result
    }

    // ---- builtin modules ----------------------------------------------

    fn b_cube(&mut self, args: &[Arg]) -> EResult<Node> {
        let m = self.bind_named(&["size", "center"], args)?;
        let size = match m.get("size") {
            Some(Value::Number(n)) => [*n, *n, *n],
            Some(v @ Value::Vector(_)) => v.as_vec3().unwrap_or([1.0, 1.0, 1.0]),
            _ => [1.0, 1.0, 1.0],
        };
        let center = m.get("center").map(Value::truthy).unwrap_or(false);
        Ok(Node::Cube { size, center })
    }

    fn b_sphere(&mut self, args: &[Arg]) -> EResult<Node> {
        let m = self.bind_named(&["r"], args)?;
        let r = if let Some(d) = m.get("d").and_then(Value::as_number) {
            d / 2.0
        } else {
            m.get("r").and_then(Value::as_number).unwrap_or(1.0)
        };
        Ok(Node::Sphere {
            r,
            frags: self.frag_spec(&m),
        })
    }

    fn b_cylinder(&mut self, args: &[Arg]) -> EResult<Node> {
        let m = self.bind_named(&["h", "r1", "r2"], args)?;
        let h = m.get("h").and_then(Value::as_number).unwrap_or(1.0);

        // r / d apply to both ends; r1/r2/d1/d2 override per end.
        let base_r = m
            .get("d")
            .and_then(Value::as_number)
            .map(|d| d / 2.0)
            .or_else(|| m.get("r").and_then(Value::as_number));

        let r1 = m
            .get("d1")
            .and_then(Value::as_number)
            .map(|d| d / 2.0)
            .or_else(|| m.get("r1").and_then(Value::as_number))
            .or(base_r)
            .unwrap_or(1.0);
        let r2 = m
            .get("d2")
            .and_then(Value::as_number)
            .map(|d| d / 2.0)
            .or_else(|| m.get("r2").and_then(Value::as_number))
            .or(base_r)
            .unwrap_or(1.0);

        let center = m.get("center").map(Value::truthy).unwrap_or(false);
        Ok(Node::Cylinder {
            h,
            r1,
            r2,
            center,
            frags: self.frag_spec(&m),
        })
    }

    fn b_polyhedron(&mut self, args: &[Arg]) -> EResult<Node> {
        let m = self.bind_named(&["points", "faces", "convexity"], args)?;
        let points: Vec<Vec3> = match m.get("points") {
            Some(Value::Vector(v)) => v.iter().map(value_to_point3).collect(),
            _ => Vec::new(),
        };
        // `faces` (current) or `triangles` (legacy).
        let faces_val = m.get("faces").or_else(|| m.get("triangles"));
        let faces: Vec<Vec<u32>> = match faces_val {
            Some(Value::Vector(v)) => v.iter().map(value_to_face).collect(),
            _ => Vec::new(),
        };
        Ok(Node::Polyhedron { points, faces })
    }

    /// Resolve the fragment spec from call-site `$fn/$fa/$fs` args, falling back
    /// to the ambient special variables.
    fn frag_spec(&self, m: &HashMap<String, Value>) -> FragmentSpec {
        let pick = |key: &str, default: f64| -> f64 {
            m.get(key)
                .and_then(Value::as_number)
                .unwrap_or_else(|| self.lookup_var(key).as_number().unwrap_or(default))
        };
        FragmentSpec {
            fn_: pick("$fn", 0.0),
            fa: pick("$fa", 12.0),
            fs: pick("$fs", 2.0),
        }
    }

    fn transform(
        &mut self,
        args: &[Arg],
        children: &[Stmt],
        kind: TransformKind,
    ) -> EResult<Node> {
        let child = Node::group(self.eval_children(children)?);
        if matches!(child, Node::Empty) {
            return Ok(Node::Empty);
        }
        let v = self.first_positional(args)?;
        let node = match kind {
            TransformKind::Translate => Node::Translate {
                v: v.as_vec3().unwrap_or([0.0, 0.0, 0.0]),
                child: Box::new(child),
            },
            TransformKind::Rotate => {
                let deg = match &v {
                    Value::Number(n) => [0.0, 0.0, *n],
                    _ => v.as_vec3().unwrap_or([0.0, 0.0, 0.0]),
                };
                Node::Rotate {
                    deg,
                    child: Box::new(child),
                }
            }
            TransformKind::Scale => {
                let s = scale_vec3(&v);
                Node::Scale {
                    v: s,
                    child: Box::new(child),
                }
            }
        };
        Ok(node)
    }

    /// Format and record an `echo(...)`; shared by the module and expression forms.
    fn do_echo(&mut self, args: &[Arg]) -> EResult<()> {
        let mut parts = Vec::new();
        for a in args {
            let v = self.eval_expr(&a.value)?;
            match &a.name {
                Some(n) => parts.push(format!("{} = {}", n, v.repr())),
                None => parts.push(v.repr()),
            }
        }
        self.echoes.push(format!("ECHO: {}", parts.join(", ")));
        Ok(())
    }

    fn b_echo(&mut self, args: &[Arg], children: &[Stmt]) -> EResult<Node> {
        self.do_echo(args)?;
        Ok(Node::group(self.eval_children(children)?))
    }

    /// Assert semantics shared by the module and expression forms; returns
    /// whether the assertion passed (errors on failure).
    fn do_assert(&mut self, args: &[Arg]) -> EResult<()> {
        let cond = self
            .first_positional(args)
            .unwrap_or(Value::Undef)
            .truthy();
        if !cond {
            let msg = args
                .iter()
                .nth(1)
                .map(|a| self.eval_expr(&a.value))
                .transpose()?
                .map(|v| v.to_str())
                .unwrap_or_default();
            return err(format!("Assertion failed: {msg}"));
        }
        Ok(())
    }

    fn b_assert(&mut self, args: &[Arg], children: &[Stmt]) -> EResult<Node> {
        self.do_assert(args)?;
        Ok(Node::group(self.eval_children(children)?))
    }

    // ---- argument binding ---------------------------------------------

    /// Bind arguments by the given positional parameter names, honoring named
    /// args (including out-of-band `$fn`-style and `d`/`r1` overrides).
    fn bind_named(&mut self, positional: &[&str], args: &[Arg]) -> EResult<HashMap<String, Value>> {
        let mut map = HashMap::new();
        let mut pos = 0;
        for a in args {
            let v = self.eval_expr(&a.value)?;
            match &a.name {
                Some(n) => {
                    map.insert(n.clone(), v);
                }
                None => {
                    if let Some(name) = positional.get(pos) {
                        map.insert((*name).to_string(), v);
                    }
                    pos += 1;
                }
            }
        }
        Ok(map)
    }

    fn bind_params(&mut self, params: &[Param], args: &[Arg]) -> EResult<HashMap<String, Value>> {
        let mut map = HashMap::new();
        for p in params {
            if let Some(d) = &p.default {
                let v = self.eval_expr(d)?;
                map.insert(p.name.clone(), v);
            }
        }
        let mut pos = 0;
        for a in args {
            let v = self.eval_expr(&a.value)?;
            match &a.name {
                Some(n) => {
                    map.insert(n.clone(), v);
                }
                None => {
                    if let Some(p) = params.get(pos) {
                        map.insert(p.name.clone(), v);
                    }
                    pos += 1;
                }
            }
        }
        Ok(map)
    }

    fn first_positional(&mut self, args: &[Arg]) -> EResult<Value> {
        for a in args {
            if a.name.is_none() {
                return self.eval_expr(&a.value);
            }
        }
        Ok(Value::Undef)
    }

    // ---- expressions ---------------------------------------------------

    fn eval_expr(&mut self, expr: &Expr) -> EResult<Value> {
        match expr {
            Expr::Number(n) => Ok(Value::Number(*n)),
            Expr::Bool(b) => Ok(Value::Bool(*b)),
            Expr::Str(s) => Ok(Value::Str(s.clone())),
            Expr::Undef => Ok(Value::Undef),
            Expr::Ident(name) => Ok(self.lookup_var(name)),
            Expr::Vector(elems) => {
                let mut out = Vec::with_capacity(elems.len());
                for e in elems {
                    self.eval_list_elem(e, &mut out)?;
                }
                Ok(value::vector(out))
            }
            Expr::Range { start, step, end } => {
                let s = self.eval_expr(start)?.as_number().unwrap_or(f64::NAN);
                let e = self.eval_expr(end)?.as_number().unwrap_or(f64::NAN);
                match step {
                    // 3-arg range: kept as written (empty if step direction
                    // contradicts start/end).
                    Some(x) => {
                        let st = self.eval_expr(x)?.as_number().unwrap_or(1.0);
                        Ok(Value::Range { start: s, step: st, end: e })
                    }
                    // 2-arg range: OpenSCAD normalizes to ascending, step 1
                    // (so `[5:2]` becomes `[2:1:5]`).
                    None => {
                        let (lo, hi) = if s <= e { (s, e) } else { (e, s) };
                        Ok(Value::Range { start: lo, step: 1.0, end: hi })
                    }
                }
            }
            Expr::Unary { op, expr } => {
                let v = self.eval_expr(expr)?;
                Ok(value::unary(*op, v))
            }
            // `&&` / `||` short-circuit (the right side may assert or error).
            Expr::Binary { op: BinOp::And, lhs, rhs } => {
                if !self.eval_expr(lhs)?.truthy() {
                    Ok(Value::Bool(false))
                } else {
                    Ok(Value::Bool(self.eval_expr(rhs)?.truthy()))
                }
            }
            Expr::Binary { op: BinOp::Or, lhs, rhs } => {
                if self.eval_expr(lhs)?.truthy() {
                    Ok(Value::Bool(true))
                } else {
                    Ok(Value::Bool(self.eval_expr(rhs)?.truthy()))
                }
            }
            Expr::Binary { op, lhs, rhs } => {
                let l = self.eval_expr(lhs)?;
                let r = self.eval_expr(rhs)?;
                Ok(value::binary(*op, l, r))
            }
            Expr::Ternary { cond, then, els } => {
                if self.eval_expr(cond)?.truthy() {
                    self.eval_expr(then)
                } else {
                    self.eval_expr(els)
                }
            }
            Expr::Index { base, index } => {
                let b = self.eval_expr(base)?;
                let i = self.eval_expr(index)?;
                Ok(index_value(&b, &i))
            }
            Expr::Member { base, field } => {
                let b = self.eval_expr(base)?;
                let idx = match field.as_str() {
                    "x" => 0,
                    "y" => 1,
                    "z" => 2,
                    _ => return Ok(Value::Undef),
                };
                Ok(index_value(&b, &Value::Number(idx as f64)))
            }
            Expr::Let { bindings, body } => {
                self.push_scope();
                for (n, e) in bindings {
                    let v = self.eval_expr(e)?;
                    self.set_var(n, v);
                }
                let r = self.eval_expr(body);
                self.pop_scope();
                r
            }
            Expr::Call { name, args } => self.eval_call(name, args),
            Expr::FunctionLiteral { params, body } => Ok(Value::Function(Rc::new(FnClosure {
                params: params.clone(),
                body: (**body).clone(),
                env: self.scopes.clone(),
            }))),
            Expr::CallValue { callee, args } => {
                let c = self.eval_expr(callee)?;
                if let Value::Function(f) = c {
                    self.call_function(&f, args)
                } else {
                    Ok(Value::Undef)
                }
            }
            Expr::Echo { args, body } => {
                self.do_echo(args)?;
                self.eval_expr(body)
            }
            Expr::Assert { args, body } => {
                self.do_assert(args)?;
                self.eval_expr(body)
            }
        }
    }

    /// Call a function closure: arguments are bound in the caller's scope, then
    /// the body runs in the closure's captured lexical environment.
    ///
    /// Self-tail-calls are eliminated: when the body reduces (through ternaries
    /// and `let`s) to a call of the same function in tail position, the frame is
    /// reused in a loop instead of recursing, so accumulator-style recursion
    /// runs to arbitrary depth without overflowing the (small, on wasm) stack.
    fn call_function(&mut self, f: &Rc<FnClosure>, args: &[Arg]) -> EResult<Value> {
        if self.depth >= MAX_CALL_DEPTH {
            return err("maximum call depth exceeded");
        }
        let mut bound = self.bind_params(&f.params, args)?;
        self.depth += 1;
        let mut iters = 0usize;
        let result = loop {
            let saved = std::mem::replace(&mut self.scopes, f.env.clone());
            self.push_scope();
            for (k, v) in bound.drain() {
                self.set_var(&k, v);
            }
            let tail = self.eval_tail(&f.body, f);
            self.pop_scope();
            self.scopes = saved;
            match tail {
                Err(e) => break Err(e),
                Ok(TailResult::Value(v)) => break Ok(v),
                Ok(TailResult::TailCall(next)) => {
                    bound = next;
                    iters += 1;
                    if iters > MAX_RANGE_ITERS {
                        break err("tail recursion exceeded iteration limit");
                    }
                }
            }
        };
        self.depth -= 1;
        result
    }

    /// Evaluate `expr` in tail position relative to function `f`. Returns either
    /// a final value or a request to tail-call `f` with fresh arguments (already
    /// evaluated in the current frame).
    fn eval_tail(&mut self, expr: &Expr, f: &Rc<FnClosure>) -> EResult<TailResult> {
        match expr {
            Expr::Ternary { cond, then, els } => {
                let branch = if self.eval_expr(cond)?.truthy() { then } else { els };
                self.eval_tail(branch, f)
            }
            Expr::Let { bindings, body } => {
                self.push_scope();
                for (n, e) in bindings {
                    let v = self.eval_expr(e)?;
                    self.set_var(n, v);
                }
                let r = self.eval_tail(body, f);
                self.pop_scope();
                r
            }
            Expr::Echo { args, body } => {
                self.do_echo(args)?;
                self.eval_tail(body, f)
            }
            Expr::Assert { args, body } => {
                self.do_assert(args)?;
                self.eval_tail(body, f)
            }
            Expr::Call { name, args } => {
                // A self-call in tail position becomes a loop iteration.
                if let Some(g) = self.lookup_func(name) {
                    if Rc::ptr_eq(&g, f) {
                        let next = self.bind_params(&f.params, args)?;
                        return Ok(TailResult::TailCall(next));
                    }
                }
                Ok(TailResult::Value(self.eval_expr(expr)?))
            }
            _ => Ok(TailResult::Value(self.eval_expr(expr)?)),
        }
    }

    fn eval_list_elem(&mut self, el: &ListElem, out: &mut Vec<Value>) -> EResult<()> {
        match el {
            ListElem::Item(e) => {
                out.push(self.eval_expr(e)?);
            }
            ListElem::Each(inner) => {
                // Evaluate the operand element, then splice each produced value.
                let mut temp = Vec::new();
                self.eval_list_elem(inner, &mut temp)?;
                for v in temp {
                    match v {
                        Value::Vector(xs) => out.extend(xs.iter().cloned()),
                        r @ Value::Range { .. } => out.extend(iter_values(&r)?),
                        Value::Undef => {}
                        other => out.push(other),
                    }
                }
            }
            ListElem::For { bindings, body } => {
                self.lc_for_rec(bindings, body, out)?;
            }
            ListElem::CFor {
                init,
                cond,
                update,
                body,
            } => {
                self.push_scope();
                for (n, e) in init {
                    let v = self.eval_expr(e)?;
                    self.set_var(n, v);
                }
                let mut iters = 0usize;
                loop {
                    if !self.eval_expr(cond)?.truthy() {
                        break;
                    }
                    self.eval_list_elem(body, out)?;
                    // Updates are applied sequentially, each seeing prior updates
                    // in the same clause (matches OpenSCAD's accumulator form).
                    for (n, e) in update {
                        let v = self.eval_expr(e)?;
                        self.set_var(n, v);
                    }
                    iters += 1;
                    if iters > MAX_RANGE_ITERS {
                        self.pop_scope();
                        return err("C-style for exceeded iteration limit");
                    }
                }
                self.pop_scope();
            }
            ListElem::Let { bindings, body } => {
                self.push_scope();
                for (n, e) in bindings {
                    let val = self.eval_expr(e)?;
                    self.set_var(n, val);
                }
                let r = self.eval_list_elem(body, out);
                self.pop_scope();
                r?;
            }
            ListElem::If { cond, then, els } => {
                if self.eval_expr(cond)?.truthy() {
                    self.eval_list_elem(then, out)?;
                } else if let Some(e) = els {
                    self.eval_list_elem(e, out)?;
                }
            }
        }
        Ok(())
    }

    fn lc_for_rec(
        &mut self,
        bindings: &[(String, Expr)],
        body: &ListElem,
        out: &mut Vec<Value>,
    ) -> EResult<()> {
        if bindings.is_empty() {
            return self.eval_list_elem(body, out);
        }
        let (name, expr) = &bindings[0];
        let vals = iter_values(&self.eval_expr(expr)?)?;
        for v in vals {
            self.push_scope();
            self.set_var(name, v);
            let r = self.lc_for_rec(&bindings[1..], body, out);
            self.pop_scope();
            r?;
        }
        Ok(())
    }

    fn eval_call(&mut self, name: &str, args: &[Arg]) -> EResult<Value> {
        // User-defined function?
        if let Some(def) = self.lookup_func(name) {
            return self.call_function(&def, args);
        }
        // A variable holding a function value?
        if let Value::Function(f) = self.lookup_var(name) {
            return self.call_function(&f, args);
        }
        // Builtins.
        let vals: Vec<Value> = args
            .iter()
            .map(|a| self.eval_expr(&a.value))
            .collect::<EResult<_>>()?;
        Ok(builtin_fn(name, &vals, &mut self.warnings))
    }
}

enum TransformKind {
    Translate,
    Rotate,
    Scale,
}

fn scale_vec3(v: &Value) -> Vec3 {
    match v {
        Value::Number(n) => [*n, *n, *n],
        Value::Vector(xs) => {
            let get = |i: usize| xs.get(i).and_then(Value::as_number).unwrap_or(1.0);
            [get(0), get(1), get(2)]
        }
        _ => [1.0, 1.0, 1.0],
    }
}

fn value_to_point3(v: &Value) -> Vec3 {
    match v {
        Value::Vector(c) => {
            let g = |i: usize| c.get(i).and_then(Value::as_number).unwrap_or(0.0);
            [g(0), g(1), g(2)]
        }
        _ => [0.0, 0.0, 0.0],
    }
}

fn value_to_face(v: &Value) -> Vec<u32> {
    match v {
        Value::Vector(idx) => idx
            .iter()
            .filter_map(Value::as_number)
            .map(|n| n as u32)
            .collect(),
        _ => Vec::new(),
    }
}

fn index_value(base: &Value, index: &Value) -> Value {
    match (base, index) {
        (Value::Vector(v), Value::Number(n)) => {
            let i = *n as isize;
            if i >= 0 && (i as usize) < v.len() {
                v[i as usize].clone()
            } else {
                Value::Undef
            }
        }
        (Value::Str(s), Value::Number(n)) => {
            let i = *n as isize;
            if i >= 0 {
                s.chars()
                    .nth(i as usize)
                    .map(|c| Value::Str(c.to_string()))
                    .unwrap_or(Value::Undef)
            } else {
                Value::Undef
            }
        }
        _ => Value::Undef,
    }
}

/// `chr()` — turn code point(s) into a string.
fn chr(args: &[Value]) -> Value {
    fn one(n: f64) -> String {
        u32::try_from(n as i64)
            .ok()
            .filter(|&c| c != 0) // OpenSCAD ignores code point 0
            .and_then(char::from_u32)
            .map(|c| c.to_string())
            .unwrap_or_default()
    }
    match args.first() {
        Some(Value::Number(n)) => Value::Str(one(*n)),
        Some(Value::Vector(v)) => {
            let mut s = String::new();
            for e in v.iter() {
                if let Some(n) = e.as_number() {
                    s.push_str(&one(n));
                }
            }
            Value::Str(s)
        }
        Some(r @ Value::Range { .. }) => {
            let mut s = String::new();
            if let Ok(vals) = iter_values(r) {
                for e in vals {
                    if let Some(n) = e.as_number() {
                        s.push_str(&one(n));
                    }
                }
            }
            Value::Str(s)
        }
        _ => Value::Str(String::new()),
    }
}

/// Expand a value into the sequence a `for`/comprehension iterates over.
fn iter_values(v: &Value) -> EResult<Vec<Value>> {
    match v {
        Value::Vector(xs) => Ok(xs.to_vec()),
        Value::Range { start, step, end } => {
            let mut out = Vec::new();
            let (start, step, end) = (*start, *step, *end);
            if step == 0.0 || start.is_nan() || end.is_nan() || step.is_nan() {
                return Ok(out);
            }
            let mut i = 0usize;
            if step > 0.0 {
                let mut x = start;
                while x <= end + 1e-12 {
                    out.push(Value::Number(x));
                    i += 1;
                    if i > MAX_RANGE_ITERS {
                        return err("range too large");
                    }
                    x = start + step * i as f64;
                }
            } else {
                let mut x = start;
                while x >= end - 1e-12 {
                    out.push(Value::Number(x));
                    i += 1;
                    if i > MAX_RANGE_ITERS {
                        return err("range too large");
                    }
                    x = start + step * i as f64;
                }
            }
            Ok(out)
        }
        // A scalar iterates once.
        other => Ok(vec![other.clone()]),
    }
}

/// Built-in expression functions.
fn builtin_fn(name: &str, args: &[Value], warnings: &mut Vec<String>) -> Value {
    let num = |i: usize| args.get(i).and_then(Value::as_number);
    let one = |f: fn(f64) -> f64| num(0).map(|x| Value::Number(f(x))).unwrap_or(Value::Undef);

    match name {
        "abs" => one(f64::abs),
        "sign" => one(|x| {
            if x > 0.0 {
                1.0
            } else if x < 0.0 {
                -1.0
            } else {
                0.0
            }
        }),
        "floor" => one(f64::floor),
        "ceil" => one(f64::ceil),
        "round" => one(f64::round),
        "sqrt" => one(f64::sqrt),
        "exp" => one(f64::exp),
        "ln" => one(f64::ln),
        "log" => one(f64::log10),
        "sin" => one(|x| x.to_radians().sin()),
        "cos" => one(|x| x.to_radians().cos()),
        "tan" => one(|x| x.to_radians().tan()),
        "asin" => one(|x| x.asin().to_degrees()),
        "acos" => one(|x| x.acos().to_degrees()),
        "atan" => one(|x| x.atan().to_degrees()),
        "atan2" => match (num(0), num(1)) {
            (Some(y), Some(x)) => Value::Number(y.atan2(x).to_degrees()),
            _ => Value::Undef,
        },
        "pow" => match (num(0), num(1)) {
            (Some(b), Some(e)) => Value::Number(b.powf(e)),
            _ => Value::Undef,
        },
        "max" => reduce_num(args, f64::max),
        "min" => reduce_num(args, f64::min),
        "len" => match args.first() {
            Some(Value::Vector(v)) => Value::Number(v.len() as f64),
            Some(Value::Str(s)) => Value::Number(s.chars().count() as f64),
            _ => Value::Undef,
        },
        "norm" => match args.first() {
            Some(Value::Vector(v)) => {
                let sum: f64 = v.iter().filter_map(Value::as_number).map(|x| x * x).sum();
                Value::Number(sum.sqrt())
            }
            _ => Value::Undef,
        },
        "cross" => cross(args),
        "concat" => {
            let mut out = Vec::new();
            for a in args {
                match a {
                    Value::Vector(v) => out.extend(v.iter().cloned()),
                    other => out.push(other.clone()),
                }
            }
            value::vector(out)
        }
        "is_undef" => Value::Bool(matches!(args.first(), Some(Value::Undef) | None)),
        "is_num" => Value::Bool(matches!(args.first(), Some(Value::Number(_)))),
        "is_bool" => Value::Bool(matches!(args.first(), Some(Value::Bool(_)))),
        "is_string" => Value::Bool(matches!(args.first(), Some(Value::Str(_)))),
        "is_list" => Value::Bool(matches!(args.first(), Some(Value::Vector(_)))),
        "is_function" => Value::Bool(matches!(args.first(), Some(Value::Function(_)))),
        "is_range" => Value::Bool(matches!(args.first(), Some(Value::Range { .. }))),
        "version" => value::vector(vec![
            Value::Number(2021.0),
            Value::Number(1.0),
            Value::Number(0.0),
        ]),
        "version_num" => Value::Number(20210100.0),
        "lookup" => lookup(args),
        "search" => search(args),
        "str" => {
            let mut s = String::new();
            for a in args {
                s.push_str(&a.to_str());
            }
            Value::Str(s)
        }
        "chr" => chr(args),
        "ord" => match args.first() {
            Some(Value::Str(s)) => match s.chars().next() {
                Some(c) => Value::Number(c as u32 as f64),
                None => Value::Undef,
            },
            _ => Value::Undef,
        },
        _ => {
            warnings.push(format!("Ignoring unknown function '{name}'"));
            Value::Undef
        }
    }
}

fn reduce_num(args: &[Value], f: fn(f64, f64) -> f64) -> Value {
    // max(v) over a single vector, or max(a,b,c,...) over scalars.
    let nums: Vec<f64> = if let [Value::Vector(v)] = args {
        v.iter().filter_map(Value::as_number).collect()
    } else {
        args.iter().filter_map(Value::as_number).collect()
    };
    match nums.split_first() {
        Some((first, rest)) => Value::Number(rest.iter().fold(*first, |a, b| f(a, *b))),
        None => Value::Undef,
    }
}

/// `lookup(key, table)` — linear interpolation over a `[[k, v], ...]` table.
fn lookup(args: &[Value]) -> Value {
    let Some(key) = args.first().and_then(Value::as_number) else {
        return Value::Undef;
    };
    let Some(Value::Vector(table)) = args.get(1) else {
        return Value::Undef;
    };
    let mut pairs: Vec<(f64, f64)> = table
        .iter()
        .filter_map(|e| {
            if let Value::Vector(p) = e {
                Some((p.first()?.as_number()?, p.get(1)?.as_number()?))
            } else {
                None
            }
        })
        .collect();
    if pairs.is_empty() {
        return Value::Undef;
    }
    pairs.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
    if key <= pairs[0].0 {
        return Value::Number(pairs[0].1);
    }
    let last = *pairs.last().unwrap();
    if key >= last.0 {
        return Value::Number(last.1);
    }
    for w in pairs.windows(2) {
        let (k0, v0) = w[0];
        let (k1, v1) = w[1];
        if key >= k0 && key <= k1 {
            if k1 == k0 {
                return Value::Number(v0);
            }
            let t = (key - k0) / (k1 - k0);
            return Value::Number(v0 + t * (v1 - v0));
        }
    }
    Value::Undef
}

/// `search(find, list, num_returns=1, index=0)`.
fn search(args: &[Value]) -> Value {
    let Some(find) = args.first() else {
        return Value::Undef;
    };
    let Some(list) = args.get(1) else {
        return Value::Undef;
    };
    let num_returns = args.get(2).and_then(Value::as_number).unwrap_or(1.0) as usize;
    let index = args.get(3).and_then(Value::as_number).unwrap_or(0.0) as usize;

    let entries: Vec<Value> = match list {
        Value::Vector(v) => v.to_vec(),
        Value::Str(s) => s.chars().map(|c| Value::Str(c.to_string())).collect(),
        _ => return Value::Undef,
    };
    let compare_val = |entry: &Value| -> Value {
        match entry {
            Value::Vector(row) => row.get(index).cloned().unwrap_or(Value::Undef),
            other => other.clone(),
        }
    };
    let match_indices = |needle: &Value| -> Vec<usize> {
        let mut out = Vec::new();
        for (i, e) in entries.iter().enumerate() {
            if value::value_eq(&compare_val(e), needle) {
                out.push(i);
                if num_returns != 0 && out.len() >= num_returns {
                    break;
                }
            }
        }
        out
    };
    // With the default num_returns == 1, OpenSCAD collapses each per-element
    // result to a single index (or an empty list when there is no match);
    // otherwise each result is the full list of indices.
    let pack = |idxs: Vec<usize>| -> Value {
        if num_returns == 1 {
            match idxs.first() {
                Some(i) => Value::Number(*i as f64),
                None => value::vector(Vec::new()),
            }
        } else {
            value::vector(idxs.into_iter().map(|i| Value::Number(i as f64)).collect())
        }
    };

    match find {
        // A single scalar returns a flat list of indices.
        Value::Number(_) | Value::Bool(_) => value::vector(
            match_indices(find)
                .into_iter()
                .map(|i| Value::Number(i as f64))
                .collect(),
        ),
        // A string searches per character; a list searches per element.
        Value::Str(s) => value::vector(
            s.chars()
                .map(|c| pack(match_indices(&Value::Str(c.to_string()))))
                .collect(),
        ),
        Value::Vector(vs) => {
            value::vector(vs.iter().map(|n| pack(match_indices(n))).collect())
        }
        _ => Value::Undef,
    }
}

fn cross(args: &[Value]) -> Value {
    if let (Some(Value::Vector(a)), Some(Value::Vector(b))) = (args.first(), args.get(1)) {
        let g = |v: &[Value], i: usize| v.get(i).and_then(Value::as_number).unwrap_or(f64::NAN);
        let (a0, a1, a2) = (g(a, 0), g(a, 1), g(a, 2));
        let (b0, b1, b2) = (g(b, 0), g(b, 1), g(b, 2));
        value::vector(vec![
            Value::Number(a1 * b2 - a2 * b1),
            Value::Number(a2 * b0 - a0 * b2),
            Value::Number(a0 * b1 - a1 * b0),
        ])
    } else {
        Value::Undef
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use quito_syntax::parse;

    fn eval(src: &str) -> EvalOutput {
        eval_program(&parse(src).unwrap()).unwrap()
    }

    #[test]
    fn single_cube() {
        let out = eval("cube(10);");
        assert_eq!(out.node, Node::Cube { size: [10.0, 10.0, 10.0], center: false });
    }

    #[test]
    fn difference_tree() {
        let out = eval("difference() { cube(10, center=true); sphere(6); }");
        match out.node {
            Node::Difference(children) => assert_eq!(children.len(), 2),
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn for_loop_produces_group() {
        let out = eval("for (i = [0:2]) translate([i*10, 0, 0]) cube(1);");
        match out.node {
            Node::Group(children) => assert_eq!(children.len(), 3),
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn last_assignment_wins() {
        // x is hoisted: cube should see x = 2.
        let out = eval("x = 1; cube(x); x = 2;");
        assert_eq!(out.node, Node::Cube { size: [2.0, 2.0, 2.0], center: false });
    }

    #[test]
    fn user_function() {
        let out = eval("function sq(a) = a * a; cube(sq(3));");
        assert_eq!(out.node, Node::Cube { size: [9.0, 9.0, 9.0], center: false });
    }

    #[test]
    fn user_module() {
        let out = eval("module box(s) { cube(s, center=true); } box(4);");
        assert_eq!(out.node, Node::Cube { size: [4.0, 4.0, 4.0], center: true });
    }

    #[test]
    fn echo_collected() {
        let out = eval("echo(\"hello\", 1 + 2);");
        assert_eq!(out.echoes, vec!["ECHO: \"hello\", 3".to_string()]);
    }

    #[test]
    fn recursion() {
        let out = eval("function fib(n) = n < 2 ? n : fib(n-1) + fib(n-2); cube(fib(10));");
        assert_eq!(out.node, Node::Cube { size: [55.0, 55.0, 55.0], center: false });
    }

    #[test]
    fn if_else() {
        let out = eval("if (1 > 2) cube(1); else sphere(3);");
        assert!(matches!(out.node, Node::Sphere { .. }));
    }

    #[test]
    fn cylinder_d_and_center() {
        let out = eval("cylinder(h=10, d=8, center=true);");
        match out.node {
            Node::Cylinder { h, r1, r2, center, .. } => {
                assert_eq!(h, 10.0);
                assert_eq!(r1, 4.0);
                assert_eq!(r2, 4.0);
                assert!(center);
            }
            other => panic!("unexpected: {other:?}"),
        }
    }

    fn echoes(src: &str) -> Vec<String> {
        eval(src).echoes
    }

    #[test]
    fn comprehensions() {
        assert_eq!(echoes("echo([for(i=[0:4]) i*i]);"), vec!["ECHO: [0, 1, 4, 9, 16]"]);
        assert_eq!(echoes("echo([for(i=[0:5]) if(i%2==0) i]);"), vec!["ECHO: [0, 2, 4]"]);
        assert_eq!(echoes("echo([for(i=[1:3]) let(sq=i*i) sq]);"), vec!["ECHO: [1, 4, 9]"]);
        assert_eq!(echoes("echo([each [1,2], each [3,4]]);"), vec!["ECHO: [1, 2, 3, 4]"]);
        assert_eq!(
            echoes("echo([for(i=[0:2], j=[0:2]) i*10+j]);"),
            vec!["ECHO: [0, 1, 2, 10, 11, 12, 20, 21, 22]"]
        );
    }

    #[test]
    fn string_repr_and_builtins() {
        assert_eq!(echoes("echo(\"hi\");"), vec!["ECHO: \"hi\""]);
        assert_eq!(echoes("echo(chr(65), ord(\"A\"));"), vec!["ECHO: \"A\", 65"]);
        assert_eq!(echoes("echo(str(\"n=\", 5, true));"), vec!["ECHO: \"n=5true\""]);
        assert_eq!(echoes("echo([\"a\", \"b\"]);"), vec!["ECHO: [\"a\", \"b\"]"]);
        assert_eq!(echoes("s=\"abc\"; echo(s[1]);"), vec!["ECHO: \"b\""]);
    }

    #[test]
    fn number_formatting() {
        assert_eq!(
            echoes("echo(1/3, 1e10, 1000000, 3.0, 0);"),
            vec!["ECHO: 0.333333, 1e+10, 1e+6, 3, 0"]
        );
        assert_eq!(echoes("echo(sign(-4), sign(0), sign(4));"), vec!["ECHO: -1, 0, 1"]);
    }

    #[test]
    fn function_literals() {
        assert_eq!(echoes("f = function(x) x*x; echo(f(5));"), vec!["ECHO: 25"]);
        assert_eq!(
            echoes("g = function(a,b) a+b; echo(g(3,4), is_function(g), is_function(3));"),
            vec!["ECHO: 7, true, false"]
        );
        assert_eq!(
            echoes("f = function(x) x*x; echo([for(i=[1:4]) f(i)]);"),
            vec!["ECHO: [1, 4, 9, 16]"]
        );
        // recursion through the bound name
        assert_eq!(
            echoes("h = function(n) n<=1 ? 1 : n*h(n-1); echo(h(5));"),
            vec!["ECHO: 120"]
        );
    }

    struct MapResolver(std::collections::HashMap<String, String>);
    impl FileResolver for MapResolver {
        fn load(&self, path: &str, _from: &str) -> Option<LoadedFile> {
            self.0.get(path).map(|s| LoadedFile {
                key: path.to_string(),
                source: s.clone(),
                dir: ".".to_string(),
            })
        }
    }

    #[test]
    fn include_and_use() {
        let mut files = std::collections::HashMap::new();
        files.insert(
            "lib.scad".to_string(),
            "function sqr(x)=x*x; K=7; echo(\"libran\");".to_string(),
        );
        let resolver = MapResolver(files);

        // `use` imports definitions only (no top-level echo, no variables).
        let prog = quito_syntax::parse("use <lib.scad>\necho(sqr(5), is_undef(K));").unwrap();
        let out = eval_program_with(&prog, &resolver, ".").unwrap();
        assert_eq!(out.echoes, vec!["ECHO: 25, true"]);

        // `include` splices everything (top-level echo runs; variables visible).
        let prog = quito_syntax::parse("include <lib.scad>\necho(sqr(4), K);").unwrap();
        let out = eval_program_with(&prog, &resolver, ".").unwrap();
        assert_eq!(out.echoes, vec!["ECHO: \"libran\"", "ECHO: 16, 7"]);
    }

    #[test]
    fn lexical_scoping() {
        // A global function sees the global variable, not the caller's local.
        assert_eq!(
            echoes("a=10; function f()=a; module m(){a=20; echo(f(),a);} m();"),
            vec!["ECHO: 10, 20"]
        );
        assert_eq!(
            echoes("a=10; function g(x)=x+a; module n(a){echo(g(1));} n(99);"),
            vec!["ECHO: 11"]
        );
        // A function literal closes over its defining scope.
        assert_eq!(
            echoes("x=5; lit=function() x; module m(){x=99; echo(lit());} m();"),
            vec!["ECHO: 5"]
        );
        // `$` variables are dynamically scoped through module calls.
        assert_eq!(
            echoes("$fn=8; module r(){echo($fn);} module s(){$fn=16; r();} s(); r();"),
            vec!["ECHO: 16", "ECHO: 8"]
        );
    }

    #[test]
    fn power_operator() {
        // right-associative, binds tighter than unary and *
        assert_eq!(
            echoes("echo(2^10, -2^2, 2^-3, 2^3^2, 3*2^2);"),
            vec!["ECHO: 1024, -4, 0.125, 512, 12"]
        );
    }

    #[test]
    fn cstyle_for() {
        assert_eq!(
            echoes("echo([for(k=0,s=0;k<=3;k=k+1,s=s+k) s]);"),
            vec!["ECHO: [0, 1, 3, 6]"]
        );
        assert_eq!(
            echoes("echo([for(a=1,b=1;a<=5;a=a+1,b=b*a) b]);"),
            vec!["ECHO: [1, 2, 6, 24, 120]"]
        );
    }

    #[test]
    fn polyhedron_node() {
        let out = eval(
            "polyhedron(points=[[0,0,0],[1,0,0],[0,1,0],[0,0,1]], \
             faces=[[0,1,2],[0,1,3],[1,2,3],[0,2,3]]);",
        );
        match out.node {
            Node::Polyhedron { points, faces } => {
                assert_eq!(points.len(), 4);
                assert_eq!(faces.len(), 4);
                assert_eq!(points[1], [1.0, 0.0, 0.0]);
            }
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn module_children() {
        let out = eval("module m() { translate([1,0,0]) children(); } m() { cube(2); sphere(3); }");
        match out.node {
            Node::Translate { child, .. } => match *child {
                Node::Group(c) => assert_eq!(c.len(), 2),
                other => panic!("unexpected: {other:?}"),
            },
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn children_count_and_index() {
        let out = eval(
            "module m() { echo($children); children(1); } m() { cube(1); sphere(2); cube(3); }",
        );
        assert_eq!(out.echoes, vec!["ECHO: 3"]);
        assert!(matches!(out.node, Node::Sphere { .. }));
    }

    #[test]
    fn list_builtins() {
        assert_eq!(echoes("echo(search(3,[1,2,3,4]));"), vec!["ECHO: [2]"]);
        assert_eq!(echoes("echo(search(\"b\",\"abcabc\"));"), vec!["ECHO: [1]"]);
        assert_eq!(
            echoes("echo(lookup(2.5,[[0,0],[1,10],[2,20],[3,30]]));"),
            vec!["ECHO: 25"]
        );
        assert_eq!(
            echoes("echo(is_undef(undef), is_list([1]), is_num(1), is_string(\"s\"));"),
            vec!["ECHO: true, true, true, true"]
        );
    }

    #[test]
    fn disable_modifier() {
        let out = eval("union() { cube(1); *sphere(5); }");
        match out.node {
            Node::Union(children) => assert_eq!(children.len(), 1),
            other => panic!("unexpected: {other:?}"),
        }
    }
}
