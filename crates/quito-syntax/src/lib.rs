//! Lexing and parsing for the OpenSCAD language (M0 subset).
//!
//! Clean-room: grammar reconstructed from public documentation and black-box
//! observation of the OpenSCAD CLI. No OpenSCAD (GPL) source is consulted.

pub mod ast;
pub mod customizer;
pub mod lexer;
pub mod parser;

pub use ast::*;

/// A lexing or parsing error, carrying a source byte span.
#[derive(Debug, Clone, thiserror::Error)]
#[error("{message}")]
pub struct SyntaxError {
    pub message: String,
    pub span: std::ops::Range<usize>,
}

impl SyntaxError {
    pub fn new(message: String, span: std::ops::Range<usize>) -> Self {
        SyntaxError { message, span }
    }
}

/// Parse a complete program from source.
pub fn parse(src: &str) -> Result<Program, SyntaxError> {
    let tokens = lexer::lex(src)?;
    let mut parser = parser::Parser::new(tokens, src);
    parser.parse_program()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simple_primitive() {
        let prog = parse("cube(10);").unwrap();
        assert_eq!(prog.len(), 1);
        match &prog[0] {
            Stmt::ModuleCall { name, args, children, .. } => {
                assert_eq!(name, "cube");
                assert_eq!(args.len(), 1);
                assert!(children.is_empty());
            }
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn nested_transforms() {
        let prog = parse("translate([1,2,3]) rotate([0,0,45]) cube(2, center=true);").unwrap();
        assert_eq!(prog.len(), 1);
        match &prog[0] {
            Stmt::ModuleCall { name, children, .. } => {
                assert_eq!(name, "translate");
                assert_eq!(children.len(), 1);
                match &children[0] {
                    Stmt::ModuleCall { name, .. } => assert_eq!(name, "rotate"),
                    other => panic!("unexpected: {other:?}"),
                }
            }
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn boolean_block() {
        let prog = parse("difference() { cube(10); sphere(6); }").unwrap();
        match &prog[0] {
            Stmt::ModuleCall { name, children, .. } => {
                assert_eq!(name, "difference");
                assert_eq!(children.len(), 2);
            }
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn for_and_if() {
        let prog = parse("for (i = [0:2:10]) if (i > 2) translate([i,0,0]) cube(1);").unwrap();
        match &prog[0] {
            Stmt::For { bindings, body } => {
                assert_eq!(bindings.len(), 1);
                assert_eq!(bindings[0].0, "i");
                assert_eq!(body.len(), 1);
            }
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn expression_precedence() {
        // 1 + 2 * 3 == 7  should parse as 1 + (2*3)
        let prog = parse("x = 1 + 2 * 3;").unwrap();
        match &prog[0] {
            Stmt::Assign { value, .. } => match value {
                Expr::Binary { op: BinOp::Add, rhs, .. } => {
                    assert!(matches!(**rhs, Expr::Binary { op: BinOp::Mul, .. }));
                }
                other => panic!("unexpected: {other:?}"),
            },
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn module_and_function_defs() {
        let prog = parse(
            "function sq(x) = x * x; module ring(r) { cylinder(r=r, h=1); } ring(sq(2));",
        )
        .unwrap();
        assert_eq!(prog.len(), 3);
        assert!(matches!(prog[0], Stmt::FunctionDef { .. }));
        assert!(matches!(prog[1], Stmt::ModuleDef { .. }));
    }

    #[test]
    fn modifier_prefix() {
        let prog = parse("*cube(1); #sphere(2);").unwrap();
        match &prog[0] {
            Stmt::ModuleCall { modifier, .. } => {
                assert_eq!(*modifier, Some(Modifier::Disable));
            }
            other => panic!("unexpected: {other:?}"),
        }
    }
}
