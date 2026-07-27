//! Runtime values and their operational semantics.

use quito_syntax::ast::{BinOp, UnOp};

/// A runtime value.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Undef,
    Bool(bool),
    Number(f64),
    Str(String),
    Vector(Vec<Value>),
    /// A numeric range `[start : step : end]`.
    Range { start: f64, step: f64, end: f64 },
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

    /// Format a value the way `echo` would (approximate; the exact 5-sig-digit
    /// oracle formatting lands in M2 with the echo-oracle harness).
    pub fn to_echo_string(&self) -> String {
        match self {
            Value::Undef => "undef".to_string(),
            Value::Bool(b) => b.to_string(),
            Value::Number(n) => format_number(*n),
            Value::Str(s) => s.clone(),
            Value::Vector(v) => {
                let parts: Vec<String> = v.iter().map(|e| e.to_echo_string()).collect();
                format!("[{}]", parts.join(", "))
            }
            Value::Range { start, step, end } => {
                format!(
                    "[{} : {} : {}]",
                    format_number(*start),
                    format_number(*step),
                    format_number(*end)
                )
            }
        }
    }
}

/// Format a number roughly like OpenSCAD (up to 6 significant digits, no
/// trailing zeros, `inf`/`nan` spelled out).
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
    let s = format!("{:.6}", n);
    // trim to significant form
    let g = format!("{}", n);
    // Prefer the shorter of the plain and fixed representations that round-trips.
    if g.parse::<f64>() == Ok(n) && g.len() <= s.len() {
        g
    } else {
        let trimmed = s.trim_end_matches('0').trim_end_matches('.');
        trimmed.to_string()
    }
}

/// Apply a unary operator.
pub fn unary(op: UnOp, v: Value) -> Value {
    match op {
        UnOp::Pos => v,
        UnOp::Neg => match v {
            Value::Number(n) => Value::Number(-n),
            Value::Vector(xs) => Value::Vector(xs.into_iter().map(|e| unary(UnOp::Neg, e)).collect()),
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
            Vector(zip_map(a, b, BinOp::Add))
        }
        (BinOp::Sub, Vector(a), Vector(b)) if a.len() == b.len() => {
            Vector(zip_map(a, b, BinOp::Sub))
        }

        // scalar * vector, vector * scalar
        (BinOp::Mul, Number(s), Vector(v)) | (BinOp::Mul, Vector(v), Number(s)) => {
            Vector(v.into_iter().map(|e| binary(BinOp::Mul, Number(s), e)).collect())
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
            Vector(v.into_iter().map(|e| binary(BinOp::Div, e, Number(s))).collect())
        }

        _ => Undef,
    }
}

fn zip_map(a: Vec<Value>, b: Vec<Value>, op: BinOp) -> Vec<Value> {
    a.into_iter().zip(b).map(|(x, y)| binary(op, x, y)).collect()
}

fn value_eq(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Number(x), Value::Number(y)) => x == y,
        (Value::Bool(x), Value::Bool(y)) => x == y,
        (Value::Str(x), Value::Str(y)) => x == y,
        (Value::Undef, Value::Undef) => true,
        (Value::Vector(x), Value::Vector(y)) => {
            x.len() == y.len() && x.iter().zip(y).all(|(p, q)| value_eq(p, q))
        }
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
