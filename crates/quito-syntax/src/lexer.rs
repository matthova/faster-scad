//! logos-based lexer for the OpenSCAD language (M0 subset).

use logos::Logos;

/// Process a double-quoted string literal into its unescaped contents.
fn unescape(lex: &mut logos::Lexer<Token>) -> String {
    let s = lex.slice();
    // strip surrounding quotes
    let inner = &s[1..s.len() - 1];
    let mut out = String::with_capacity(inner.len());
    let mut chars = inner.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some('n') => out.push('\n'),
                Some('t') => out.push('\t'),
                Some('r') => out.push('\r'),
                Some('\\') => out.push('\\'),
                Some('"') => out.push('"'),
                Some(other) => {
                    out.push('\\');
                    out.push(other);
                }
                None => out.push('\\'),
            }
        } else {
            out.push(c);
        }
    }
    out
}

#[derive(Logos, Debug, Clone, PartialEq)]
#[logos(skip r"[ \t\r\n\f]+")]
#[logos(skip r"//[^\n]*")]
#[logos(skip r"/\*[^*]*\*+([^/*][^*]*\*+)*/")]
pub enum Token {
    // literals
    #[regex(r"(?:[0-9]+\.?[0-9]*|\.[0-9]+)(?:[eE][+-]?[0-9]+)?", |lex| lex.slice().parse::<f64>().ok())]
    Number(f64),

    #[regex(r#""(?:[^"\\]|\\.)*""#, unescape)]
    Str(String),

    #[token("true")]
    True,
    #[token("false")]
    False,
    #[token("undef")]
    Undef,

    // keywords
    #[token("module")]
    Module,
    #[token("function")]
    Function,
    #[token("if")]
    If,
    #[token("else")]
    Else,
    #[token("for")]
    For,
    #[token("let")]
    Let,

    // identifiers (including `$special` variables)
    #[regex(r"\$?[A-Za-z_][A-Za-z0-9_]*", |lex| lex.slice().to_owned())]
    Ident(String),

    // punctuation
    #[token("(")]
    LParen,
    #[token(")")]
    RParen,
    #[token("{")]
    LBrace,
    #[token("}")]
    RBrace,
    #[token("[")]
    LBracket,
    #[token("]")]
    RBracket,
    #[token(";")]
    Semi,
    #[token(",")]
    Comma,
    #[token(":")]
    Colon,
    #[token(".")]
    Dot,

    // operators
    #[token("=")]
    Assign,
    #[token("==")]
    Eq,
    #[token("!=")]
    Ne,
    #[token("<")]
    Lt,
    #[token("<=")]
    Le,
    #[token(">")]
    Gt,
    #[token(">=")]
    Ge,
    #[token("+")]
    Plus,
    #[token("-")]
    Minus,
    #[token("*")]
    Star,
    #[token("/")]
    Slash,
    #[token("%")]
    Percent,
    #[token("!")]
    Bang,
    #[token("#")]
    Hash,
    #[token("&&")]
    And,
    #[token("||")]
    Or,
    #[token("?")]
    Question,
}

/// A token together with its source byte span.
#[derive(Debug, Clone, PartialEq)]
pub struct Spanned {
    pub token: Token,
    pub span: std::ops::Range<usize>,
}

/// Lex an entire source string. Returns an error at the first invalid token.
pub fn lex(src: &str) -> Result<Vec<Spanned>, crate::SyntaxError> {
    let mut out = Vec::new();
    let mut lexer = Token::lexer(src);
    while let Some(res) = lexer.next() {
        match res {
            Ok(token) => out.push(Spanned {
                token,
                span: lexer.span(),
            }),
            Err(_) => {
                return Err(crate::SyntaxError::new(
                    format!("unexpected character(s) `{}`", lexer.slice()),
                    lexer.span(),
                ))
            }
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn toks(src: &str) -> Vec<Token> {
        lex(src).unwrap().into_iter().map(|s| s.token).collect()
    }

    #[test]
    fn numbers() {
        assert_eq!(toks("1 2.5 .5 1e3 2.0e-2"), vec![
            Token::Number(1.0),
            Token::Number(2.5),
            Token::Number(0.5),
            Token::Number(1000.0),
            Token::Number(0.02),
        ]);
    }

    #[test]
    fn special_vars() {
        assert_eq!(toks("$fn $fa x"), vec![
            Token::Ident("$fn".into()),
            Token::Ident("$fa".into()),
            Token::Ident("x".into()),
        ]);
    }

    #[test]
    fn strings_and_escapes() {
        assert_eq!(toks(r#""a\nb""#), vec![Token::Str("a\nb".into())]);
    }

    #[test]
    fn comments_skipped() {
        assert_eq!(toks("1 // line\n2 /* block */ 3"), vec![
            Token::Number(1.0),
            Token::Number(2.0),
            Token::Number(3.0),
        ]);
    }

    #[test]
    fn block_comment_with_stars() {
        assert_eq!(toks("1 /** doc ** star */ 2"), vec![
            Token::Number(1.0),
            Token::Number(2.0),
        ]);
    }
}
