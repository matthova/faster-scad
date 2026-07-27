//! Recursive-descent parser producing the typed AST.

use crate::ast::*;
use crate::lexer::{Spanned, Token};
use crate::SyntaxError;

pub struct Parser {
    tokens: Vec<Spanned>,
    pos: usize,
    /// End of source, for EOF error spans.
    eof: usize,
}

type PResult<T> = Result<T, SyntaxError>;

/// Human-readable description of a token for diagnostics.
fn describe(tok: Option<&Token>) -> String {
    match tok {
        None => "end of input".to_string(),
        Some(t) => format!("{t:?}"),
    }
}

impl Parser {
    pub fn new(tokens: Vec<Spanned>, src_len: usize) -> Self {
        Parser {
            tokens,
            pos: 0,
            eof: src_len,
        }
    }

    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.pos).map(|s| &s.token)
    }

    fn peek2(&self) -> Option<&Token> {
        self.tokens.get(self.pos + 1).map(|s| &s.token)
    }

    fn span_here(&self) -> std::ops::Range<usize> {
        self.tokens
            .get(self.pos)
            .map(|s| s.span.clone())
            .unwrap_or(self.eof..self.eof)
    }

    fn at_end(&self) -> bool {
        self.pos >= self.tokens.len()
    }

    fn advance(&mut self) -> Option<Token> {
        let t = self.tokens.get(self.pos).map(|s| s.token.clone());
        if t.is_some() {
            self.pos += 1;
        }
        t
    }

    fn eat(&mut self, tok: &Token) -> bool {
        if self.peek() == Some(tok) {
            self.pos += 1;
            true
        } else {
            false
        }
    }

    fn expect(&mut self, tok: &Token) -> PResult<()> {
        if self.eat(tok) {
            Ok(())
        } else {
            Err(SyntaxError::new(
                format!("expected `{:?}`, found {}", tok, describe(self.peek())),
                self.span_here(),
            ))
        }
    }

    fn expect_ident(&mut self) -> PResult<String> {
        match self.advance() {
            Some(Token::Ident(name)) => Ok(name),
            other => Err(SyntaxError::new(
                format!("expected identifier, found {}", describe(other.as_ref())),
                self.span_here(),
            )),
        }
    }

    // ---- statements ----------------------------------------------------

    pub fn parse_program(&mut self) -> PResult<Program> {
        let mut stmts = Vec::new();
        while !self.at_end() {
            if self.eat(&Token::Semi) {
                continue;
            }
            stmts.push(self.parse_statement()?);
        }
        Ok(stmts)
    }

    fn parse_statement(&mut self) -> PResult<Stmt> {
        match self.peek() {
            Some(Token::LBrace) => Ok(Stmt::Block(self.parse_block()?)),
            Some(Token::Module) => self.parse_module_def(),
            Some(Token::Function) => self.parse_function_def(),
            Some(Token::If) => self.parse_if(),
            Some(Token::For) => self.parse_for(),
            Some(Token::Star | Token::Bang | Token::Hash | Token::Percent) => {
                let modifier = match self.advance().unwrap() {
                    Token::Star => Modifier::Disable,
                    Token::Bang => Modifier::Root,
                    Token::Hash => Modifier::Highlight,
                    Token::Percent => Modifier::Background,
                    _ => unreachable!(),
                };
                self.parse_module_call(Some(modifier))
            }
            Some(Token::Ident(_)) => {
                if self.peek2() == Some(&Token::Assign) {
                    self.parse_assign()
                } else {
                    self.parse_module_call(None)
                }
            }
            other => Err(SyntaxError::new(
                format!("unexpected token in statement position: {}", describe(other)),
                self.span_here(),
            )),
        }
    }

    /// Parse the "body" that follows a construct like `translate(...)`, `if(...)`,
    /// `for(...)`, or `module foo()`: either a `{ block }`, a single statement, or
    /// an empty `;`.
    fn parse_child_body(&mut self) -> PResult<Vec<Stmt>> {
        if self.peek() == Some(&Token::LBrace) {
            self.parse_block()
        } else if self.eat(&Token::Semi) {
            Ok(Vec::new())
        } else {
            Ok(vec![self.parse_statement()?])
        }
    }

    fn parse_block(&mut self) -> PResult<Vec<Stmt>> {
        self.expect(&Token::LBrace)?;
        let mut stmts = Vec::new();
        while self.peek() != Some(&Token::RBrace) {
            if self.at_end() {
                return Err(SyntaxError::new("unterminated block".into(), self.span_here()));
            }
            if self.eat(&Token::Semi) {
                continue;
            }
            stmts.push(self.parse_statement()?);
        }
        self.expect(&Token::RBrace)?;
        Ok(stmts)
    }

    fn parse_assign(&mut self) -> PResult<Stmt> {
        let name = self.expect_ident()?;
        self.expect(&Token::Assign)?;
        let value = self.parse_expr()?;
        self.expect(&Token::Semi)?;
        Ok(Stmt::Assign { name, value })
    }

    fn parse_module_call(&mut self, modifier: Option<Modifier>) -> PResult<Stmt> {
        let name = self.expect_ident()?;
        self.expect(&Token::LParen)?;
        let args = self.parse_args()?;
        self.expect(&Token::RParen)?;
        let children = self.parse_child_body()?;
        Ok(Stmt::ModuleCall {
            modifier,
            name,
            args,
            children,
        })
    }

    fn parse_module_def(&mut self) -> PResult<Stmt> {
        self.expect(&Token::Module)?;
        let name = self.expect_ident()?;
        self.expect(&Token::LParen)?;
        let params = self.parse_params()?;
        self.expect(&Token::RParen)?;
        let body = self.parse_child_body()?;
        Ok(Stmt::ModuleDef { name, params, body })
    }

    fn parse_function_def(&mut self) -> PResult<Stmt> {
        self.expect(&Token::Function)?;
        let name = self.expect_ident()?;
        self.expect(&Token::LParen)?;
        let params = self.parse_params()?;
        self.expect(&Token::RParen)?;
        self.expect(&Token::Assign)?;
        let body = self.parse_expr()?;
        self.expect(&Token::Semi)?;
        Ok(Stmt::FunctionDef { name, params, body })
    }

    fn parse_if(&mut self) -> PResult<Stmt> {
        self.expect(&Token::If)?;
        self.expect(&Token::LParen)?;
        let cond = self.parse_expr()?;
        self.expect(&Token::RParen)?;
        let then = self.parse_child_body()?;
        let els = if self.eat(&Token::Else) {
            self.parse_child_body()?
        } else {
            Vec::new()
        };
        Ok(Stmt::If { cond, then, els })
    }

    fn parse_for(&mut self) -> PResult<Stmt> {
        self.expect(&Token::For)?;
        self.expect(&Token::LParen)?;
        let bindings = self.parse_bindings()?;
        self.expect(&Token::RParen)?;
        let body = self.parse_child_body()?;
        Ok(Stmt::For { bindings, body })
    }

    fn parse_bindings(&mut self) -> PResult<Vec<(String, Expr)>> {
        let mut out = Vec::new();
        if self.peek() == Some(&Token::RParen) {
            return Ok(out);
        }
        loop {
            let name = self.expect_ident()?;
            self.expect(&Token::Assign)?;
            let value = self.parse_expr()?;
            out.push((name, value));
            if !self.eat(&Token::Comma) {
                break;
            }
        }
        Ok(out)
    }

    fn parse_params(&mut self) -> PResult<Vec<Param>> {
        let mut out = Vec::new();
        while self.peek() != Some(&Token::RParen) {
            let name = self.expect_ident()?;
            let default = if self.eat(&Token::Assign) {
                Some(self.parse_expr()?)
            } else {
                None
            };
            out.push(Param { name, default });
            if !self.eat(&Token::Comma) {
                break;
            }
        }
        Ok(out)
    }

    fn parse_args(&mut self) -> PResult<Vec<Arg>> {
        let mut out = Vec::new();
        while self.peek() != Some(&Token::RParen) {
            let name = if matches!(self.peek(), Some(Token::Ident(_)))
                && self.peek2() == Some(&Token::Assign)
            {
                let n = self.expect_ident()?;
                self.expect(&Token::Assign)?;
                Some(n)
            } else {
                None
            };
            let value = self.parse_expr()?;
            out.push(Arg { name, value });
            if !self.eat(&Token::Comma) {
                break;
            }
        }
        Ok(out)
    }

    // ---- expressions ---------------------------------------------------

    pub fn parse_expr(&mut self) -> PResult<Expr> {
        self.parse_ternary()
    }

    fn parse_ternary(&mut self) -> PResult<Expr> {
        let cond = self.parse_binary(1)?;
        if self.eat(&Token::Question) {
            let then = self.parse_ternary()?;
            self.expect(&Token::Colon)?;
            let els = self.parse_ternary()?;
            Ok(Expr::Ternary {
                cond: Box::new(cond),
                then: Box::new(then),
                els: Box::new(els),
            })
        } else {
            Ok(cond)
        }
    }

    fn peek_binop(&self) -> Option<(BinOp, u8)> {
        let op = match self.peek()? {
            Token::Or => (BinOp::Or, 1),
            Token::And => (BinOp::And, 2),
            Token::Eq => (BinOp::Eq, 3),
            Token::Ne => (BinOp::Ne, 3),
            Token::Lt => (BinOp::Lt, 4),
            Token::Le => (BinOp::Le, 4),
            Token::Gt => (BinOp::Gt, 4),
            Token::Ge => (BinOp::Ge, 4),
            Token::Plus => (BinOp::Add, 5),
            Token::Minus => (BinOp::Sub, 5),
            Token::Star => (BinOp::Mul, 6),
            Token::Slash => (BinOp::Div, 6),
            Token::Percent => (BinOp::Mod, 6),
            _ => return None,
        };
        Some(op)
    }

    fn parse_binary(&mut self, min_prec: u8) -> PResult<Expr> {
        let mut lhs = self.parse_unary()?;
        while let Some((op, prec)) = self.peek_binop() {
            if prec < min_prec {
                break;
            }
            self.advance();
            let rhs = self.parse_binary(prec + 1)?;
            lhs = Expr::Binary {
                op,
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
            };
        }
        Ok(lhs)
    }

    fn parse_unary(&mut self) -> PResult<Expr> {
        let op = match self.peek() {
            Some(Token::Minus) => Some(UnOp::Neg),
            Some(Token::Plus) => Some(UnOp::Pos),
            Some(Token::Bang) => Some(UnOp::Not),
            _ => None,
        };
        if let Some(op) = op {
            self.advance();
            let expr = self.parse_unary()?;
            Ok(Expr::Unary {
                op,
                expr: Box::new(expr),
            })
        } else {
            self.parse_postfix()
        }
    }

    fn parse_postfix(&mut self) -> PResult<Expr> {
        let mut base = self.parse_primary()?;
        loop {
            if self.eat(&Token::LBracket) {
                let index = self.parse_expr()?;
                self.expect(&Token::RBracket)?;
                base = Expr::Index {
                    base: Box::new(base),
                    index: Box::new(index),
                };
            } else if self.eat(&Token::Dot) {
                let field = self.expect_ident()?;
                base = Expr::Member {
                    base: Box::new(base),
                    field,
                };
            } else {
                break;
            }
        }
        Ok(base)
    }

    fn parse_primary(&mut self) -> PResult<Expr> {
        match self.peek().cloned() {
            Some(Token::Number(n)) => {
                self.advance();
                Ok(Expr::Number(n))
            }
            Some(Token::Str(s)) => {
                self.advance();
                Ok(Expr::Str(s))
            }
            Some(Token::True) => {
                self.advance();
                Ok(Expr::Bool(true))
            }
            Some(Token::False) => {
                self.advance();
                Ok(Expr::Bool(false))
            }
            Some(Token::Undef) => {
                self.advance();
                Ok(Expr::Undef)
            }
            Some(Token::Let) => self.parse_let(),
            Some(Token::Ident(name)) => {
                self.advance();
                if self.eat(&Token::LParen) {
                    let args = self.parse_args()?;
                    self.expect(&Token::RParen)?;
                    Ok(Expr::Call { name, args })
                } else {
                    Ok(Expr::Ident(name))
                }
            }
            Some(Token::LParen) => {
                self.advance();
                let e = self.parse_expr()?;
                self.expect(&Token::RParen)?;
                Ok(e)
            }
            Some(Token::LBracket) => self.parse_bracket(),
            other => Err(SyntaxError::new(
                format!("unexpected token in expression: {}", describe(other.as_ref())),
                self.span_here(),
            )),
        }
    }

    fn parse_let(&mut self) -> PResult<Expr> {
        self.expect(&Token::Let)?;
        self.expect(&Token::LParen)?;
        let bindings = self.parse_bindings()?;
        self.expect(&Token::RParen)?;
        let body = self.parse_expr()?;
        Ok(Expr::Let {
            bindings,
            body: Box::new(body),
        })
    }

    /// `[ ... ]` — either a vector literal or a range.
    fn parse_bracket(&mut self) -> PResult<Expr> {
        self.expect(&Token::LBracket)?;
        if self.eat(&Token::RBracket) {
            return Ok(Expr::Vector(Vec::new()));
        }
        let first = self.parse_expr()?;
        if self.eat(&Token::Colon) {
            // range: [start:end] or [start:step:end]
            let mid = self.parse_expr()?;
            let (step, end) = if self.eat(&Token::Colon) {
                let end = self.parse_expr()?;
                (Some(Box::new(mid)), end)
            } else {
                (None, mid)
            };
            self.expect(&Token::RBracket)?;
            Ok(Expr::Range {
                start: Box::new(first),
                step,
                end: Box::new(end),
            })
        } else {
            let mut elems = vec![first];
            while self.eat(&Token::Comma) {
                if self.peek() == Some(&Token::RBracket) {
                    break;
                }
                elems.push(self.parse_expr()?);
            }
            self.expect(&Token::RBracket)?;
            Ok(Expr::Vector(elems))
        }
    }
}
