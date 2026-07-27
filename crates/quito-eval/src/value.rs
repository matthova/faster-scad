//! Runtime values and their operational semantics.

use quito_syntax::ast::{BinOp, Expr, Param, UnOp};
use std::rc::Rc;

/// A runtime value.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Undef,
    Bool(bool),
    Number(f64),
    Str(String),
    /// A list. `Rc`-backed so cloning a value (e.g. a variable lookup of a
    /// large array) is O(1) — critical for mesh-generating scripts.
    Vector(Rc<Vec<Value>>),
    /// A numeric range `[start : step : end]`.
    Range { start: f64, step: f64, end: f64 },
    /// An anonymous function value (parameters + body). Called via
    /// dynamic scoping in M2; lexical capture is a later refinement.
    Function(Rc<(Vec<Param>, Expr)>),
}

/// Construct a list value from an owned `Vec`.
pub fn vector(v: Vec<Value>) -> Value {
    Value::Vector(Rc::new(v))
}

impl Value {
    /// OpenSCAD truthiness.
    pub fn truthy(&self) -> bool {
        match self {
            Value::Undef => false,
            Value::Bool(b) => *b,
            Value::Number(n) => *n != 0.0 && !n.is_nan(),
            Value::Str(s) => !s.is_empty(),
            Value::Vector(v) => !v.is_empty(),
            Value::Range { .. } => true,
            Value::Function(_) => true,
        }
    }

    pub fn as_number(&self) -> Option<f64> {
        match self {
            Value::Number(n) => Some(*n),
            _ => None,
        }
    }

    /// Interpret this value as a 3-component vector, broadcasting a scalar to
    /// all three components and zero-filling short vectors.
    pub fn as_vec3(&self) -> Option<[f64; 3]> {
        match self {
            Value::Number(n) => Some([*n, *n, *n]),
            Value::Vector(v) => {
                let get = |i: usize| v.get(i).and_then(Value::as_number).unwrap_or(0.0);
                if v.iter().all(|e| matches!(e, Value::Number(_))) || !v.is_empty() {
                    Some([get(0), get(1), get(2)])
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    /// The display representation used by `echo` and inside vectors: strings
    /// are quoted, matching OpenSCAD.
    pub fn repr(&self) -> String {
        match self {
            Value::Undef => "undef".to_string(),
            Value::Bool(b) => b.to_string(),
            Value::Number(n) => format_number(*n),
            Value::Str(s) => format!("\"{}\"", escape_string(s)),
            Value::Vector(v) => {
                let parts: Vec<String> = v.iter().map(|e| e.repr()).collect();
                format!("[{}]", parts.join(", "))
            }
            Value::Range { start, step, end } => format!(
                "[{} : {} : {}]",
                format_number(*start),
                format_number(*step),
                format_number(*end)
            ),
            Value::Function(_) => "function".to_string(),
        }
    }

    /// The `str()` representation: a top-level string is emitted raw (no
    /// quotes); everything else uses [`Value::repr`] (so vector elements are
    /// still quoted).
    pub fn to_str(&self) -> String {
        match self {
            Value::Str(s) => s.clone(),
            other => other.repr(),
        }
    }
}

fn escape_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\t' => out.push_str("\\t"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            _ => out.push(c),
        }
    }
    out
}

/// Format a number like OpenSCAD: C `%.6g` (6 significant digits), but with the
/// exponent written without leading zeros (`1e+6`, `1.234e-6`).
pub fn format_number(n: f64) -> String {
    if n.is_nan() {
        return "nan".to_string();
    }
    if n.is_infinite() {
        return if n < 0.0 { "-inf".into() } else { "inf".into() };
    }
    if n == 0.0 {
        return "0".to_string();
    }

    const P: i32 = 6;
    // Reliable base-10 exponent via Rust's scientific formatter.
    let sci = format!("{:e}", n);
    let exp: i32 = sci
        .split('e')
        .nth(1)
        .and_then(|e| e.parse().ok())
        .unwrap_or(0);

    if exp < -4 || exp >= P {
        // scientific, P-1 fractional mantissa digits, trailing zeros trimmed
        let s = format!("{:.*e}", (P - 1) as usize, n);
        let (mant, e) = s.split_once('e').unwrap();
        let mant = trim_frac(mant);
        let exp_num: i32 = e.parse().unwrap_or(0);
        let sign = if exp_num < 0 { "-" } else { "+" };
        format!("{mant}e{sign}{}", exp_num.abs())
    } else {
        let decimals = (P - 1 - exp).max(0) as usize;
        let s = format!("{n:.decimals$}");
        trim_frac(&s).to_string()
    }
}

/// Trim trailing zeros (and a trailing dot) from a decimal mantissa.
fn trim_frac(s: &str) -> String {
    if s.contains('.') {
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    } else {
        s.to_string()
    }
}

/// Apply a unary operator.
pub fn unary(op: UnOp, v: Value) -> Value {
    match op {
        UnOp::Pos => v,
        UnOp::Neg => match v {
            Value::Number(n) => Value::Number(-n),
            Value::Vector(xs) => {
                vector(xs.iter().map(|e| unary(UnOp::Neg, e.clone())).collect())
            }
            _ => Value::Undef,
        },
        UnOp::Not => Value::Bool(!v.truthy()),
    }
}

/// Apply a binary operator with OpenSCAD semantics (undef propagation on type
/// mismatch, elementwise vector arithmetic, vector dot product).
pub fn binary(op: BinOp, l: Value, r: Value) -> Value {
    use Value::*;
    match op {
        BinOp::And => return Bool(l.truthy() && r.truthy()),
        BinOp::Or => return Bool(l.truthy() || r.truthy()),
        BinOp::Eq => return Bool(value_eq(&l, &r)),
        BinOp::Ne => return Bool(!value_eq(&l, &r)),
        BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge => return compare(op, &l, &r),
        _ => {}
    }

    match (op, l, r) {
        // numeric
        (BinOp::Add, Number(a), Number(b)) => Number(a + b),
        (BinOp::Sub, Number(a), Number(b)) => Number(a - b),
        (BinOp::Mul, Number(a), Number(b)) => Number(a * b),
        (BinOp::Div, Number(a), Number(b)) => Number(a / b),
        (BinOp::Mod, Number(a), Number(b)) => Number(a % b),

        // vector +/- vector (elementwise, equal length)
        (BinOp::Add, Vector(a), Vector(b)) if a.len() == b.len() => {
            vector(zip_map(&a, &b, BinOp::Add))
        }
        (BinOp::Sub, Vector(a), Vector(b)) if a.len() == b.len() => {
            vector(zip_map(&a, &b, BinOp::Sub))
        }

        // scalar * vector, vector * scalar
        (BinOp::Mul, Number(s), Vector(v)) | (BinOp::Mul, Vector(v), Number(s)) => {
            vector(v.iter().map(|e| binary(BinOp::Mul, Number(s), e.clone())).collect())
        }
        // vector * vector -> dot product
        (BinOp::Mul, Vector(a), Vector(b)) if a.len() == b.len() => {
            let mut sum = 0.0;
            for (x, y) in a.iter().zip(b.iter()) {
                match (x.as_number(), y.as_number()) {
                    (Some(x), Some(y)) => sum += x * y,
                    _ => return Undef,
                }
            }
            Number(sum)
        }
        // vector / scalar
        (BinOp::Div, Vector(v), Number(s)) => {
            vector(v.iter().map(|e| binary(BinOp::Div, e.clone(), Number(s))).collect())
        }

        _ => Undef,
    }
}

fn zip_map(a: &[Value], b: &[Value], op: BinOp) -> Vec<Value> {
    a.iter()
        .zip(b)
        .map(|(x, y)| binary(op, x.clone(), y.clone()))
        .collect()
}

pub fn value_eq(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Number(x), Value::Number(y)) => x == y,
        (Value::Bool(x), Value::Bool(y)) => x == y,
        (Value::Str(x), Value::Str(y)) => x == y,
        (Value::Undef, Value::Undef) => true,
        (Value::Vector(x), Value::Vector(y)) => {
            x.len() == y.len() && x.iter().zip(y.iter()).all(|(p, q)| value_eq(p, q))
        }
        (Value::Function(x), Value::Function(y)) => Rc::ptr_eq(x, y),
        _ => false,
    }
}

fn compare(op: BinOp, a: &Value, b: &Value) -> Value {
    let ord = match (a, b) {
        (Value::Number(x), Value::Number(y)) => x.partial_cmp(y),
        _ => return Value::Undef, // mixed comparison -> undef
    };
    match ord {
        None => Value::Undef,
        Some(o) => {
            let res = match op {
                BinOp::Lt => o.is_lt(),
                BinOp::Le => o.is_le(),
                BinOp::Gt => o.is_gt(),
                BinOp::Ge => o.is_ge(),
                _ => unreachable!(),
            };
            Value::Bool(res)
        }
    }
}
