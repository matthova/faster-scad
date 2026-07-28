//! Extract user-defined symbols (modules, functions, variables) from a parsed
//! program, for completion, hover, and document-symbol requests.

use quito_syntax::ast::{Param, Program, Spanned, Stmt};

/// What kind of thing a user symbol is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SymbolKind {
    Module,
    Function,
    Variable,
}

/// A symbol defined in the document.
#[derive(Debug, Clone)]
pub struct Symbol {
    pub name: String,
    pub kind: SymbolKind,
    /// A one-line signature, e.g. `module ring(r, h)`.
    pub signature: String,
    /// Byte span of the defining statement (for go-to and document symbols).
    pub span: std::ops::Range<usize>,
}

/// Collect all user symbols from a program. Nested modules/functions (defined
/// inside another module body) are included so completion sees helper defs.
pub fn collect(prog: &Program) -> Vec<Symbol> {
    let mut out = Vec::new();
    walk(prog, &mut out);
    out
}

fn walk(stmts: &[Spanned<Stmt>], out: &mut Vec<Symbol>) {
    for s in stmts {
        match &s.node {
            Stmt::ModuleDef { name, params, body } => {
                out.push(Symbol {
                    name: name.clone(),
                    kind: SymbolKind::Module,
                    signature: format!("module {name}({})", params_sig(params)),
                    span: s.span.clone(),
                });
                walk(body, out);
            }
            Stmt::FunctionDef { name, params, .. } => {
                out.push(Symbol {
                    name: name.clone(),
                    kind: SymbolKind::Function,
                    signature: format!("function {name}({})", params_sig(params)),
                    span: s.span.clone(),
                });
            }
            Stmt::Assign { name, .. } => {
                out.push(Symbol {
                    name: name.clone(),
                    kind: SymbolKind::Variable,
                    signature: format!("{name} = …"),
                    span: s.span.clone(),
                });
            }
            // Recurse into constructs that carry child statements so nested defs
            // and loop-body modules are visible.
            Stmt::ModuleCall { children, .. } => walk(children, out),
            Stmt::For { body, .. } | Stmt::Let { body, .. } => walk(body, out),
            Stmt::If { then, els, .. } => {
                walk(then, out);
                walk(els, out);
            }
            Stmt::Block(body) => walk(body, out),
            Stmt::Include { .. } | Stmt::Use { .. } => {}
        }
    }
}

/// Format a parameter list for a signature: names, with `=…` on defaulted ones.
fn params_sig(params: &[Param]) -> String {
    params
        .iter()
        .map(|p| {
            if p.default.is_some() {
                format!("{}=…", p.name)
            } else {
                p.name.clone()
            }
        })
        .collect::<Vec<_>>()
        .join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collects_modules_functions_and_vars() {
        let prog = quito_syntax::parse(
            "width = 10;\nfunction sq(x) = x*x;\nmodule ring(r, h=1) { cylinder(r=r, h=h); }\nring(sq(2));",
        )
        .unwrap();
        let syms = collect(&prog);
        let by = |n: &str| syms.iter().find(|s| s.name == n).cloned();

        let w = by("width").unwrap();
        assert_eq!(w.kind, SymbolKind::Variable);

        let sq = by("sq").unwrap();
        assert_eq!(sq.kind, SymbolKind::Function);
        assert_eq!(sq.signature, "function sq(x)");

        let ring = by("ring").unwrap();
        assert_eq!(ring.kind, SymbolKind::Module);
        assert_eq!(ring.signature, "module ring(r, h=…)");
    }
}
