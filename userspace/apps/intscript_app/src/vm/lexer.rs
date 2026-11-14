// userspace/apps/script_app/src/vm/lexer.rs - ECMAScript Lexer
extern crate alloc;
use alloc::string::String;
use alloc::vec::Vec;
use super::token::Token;

pub struct Lexer<'a> {
    src: &'a [u8],
    pos: usize,
}

impl<'a> Lexer<'a> {
    pub fn new(source: &'a str) -> Self {
        Self {
            src: source.as_bytes(),
            pos: 0,
        }
    }

    fn peek(&self) -> Option<u8> {
        if self.pos < self.src.len() {
            Some(self.src[self.pos])
        } else {
            None
        }
    }

    fn peek_next(&self) -> Option<u8> {
        if self.pos + 1 < self.src.len() {
            Some(self.src[self.pos + 1])
        } else {
            None
        }
    }

    fn advance(&mut self) -> Option<u8> {
        if self.pos < self.src.len() {
            let b = self.src[self.pos];
            self.pos += 1;
            Some(b)
        } else {
            None
        }
    }

    fn skip_whitespace_and_comments(&mut self) {
        while let Some(b) = self.peek() {
            if b.is_ascii_whitespace() {
                self.advance();
            } else if b == b'/' && self.peek_next() == Some(b'/') {
                // Line comment
                self.advance();
                self.advance();
                while let Some(c) = self.advance() {
                    if c == b'\n' {
                        break;
                    }
                }
            } else if b == b'/' && self.peek_next() == Some(b'*') {
                // Block comment
                self.advance();
                self.advance();
                while let Some(c) = self.advance() {
                    if c == b'*' && self.peek() == Some(b'/') {
                        self.advance();
                        break;
                    }
                }
            } else {
                break;
            }
        }
    }

    pub fn tokenize(&mut self) -> Result<Vec<Token>, &'static str> {
        let mut tokens = Vec::new();

        loop {
            self.skip_whitespace_and_comments();
            let b = match self.peek() {
                Some(b) => b,
                None => {
                    tokens.push(Token::Eof);
                    break;
                }
            };

            match b {
                b'+' => { self.advance(); tokens.push(Token::Plus); }
                b'-' => { self.advance(); tokens.push(Token::Minus); }
                b'*' => { self.advance(); tokens.push(Token::Star); }
                b'/' => { self.advance(); tokens.push(Token::Slash); }
                b'.' => { self.advance(); tokens.push(Token::Dot); }
                b',' => { self.advance(); tokens.push(Token::Comma); }
                b':' => { self.advance(); tokens.push(Token::Colon); }
                b';' => { self.advance(); tokens.push(Token::Semicolon); }
                b'(' => { self.advance(); tokens.push(Token::LParen); }
                b')' => { self.advance(); tokens.push(Token::RParen); }
                b'{' => { self.advance(); tokens.push(Token::LBrace); }
                b'}' => { self.advance(); tokens.push(Token::RBrace); }
                b'[' => { self.advance(); tokens.push(Token::LBracket); }
                b']' => { self.advance(); tokens.push(Token::RBracket); }
                b'=' => {
                    self.advance();
                    if self.peek() == Some(b'=') {
                        self.advance();
                        tokens.push(Token::Eq);
                    } else if self.peek() == Some(b'>') {
                        self.advance();
                        tokens.push(Token::Arrow);
                    } else {
                        tokens.push(Token::Assign);
                    }
                }
                b'!' => {
                    self.advance();
                    if self.peek() == Some(b'=') {
                        self.advance();
                        tokens.push(Token::Neq);
                    } else {
                        return Err("Unexpected '!' without '='");
                    }
                }
                b'<' => {
                    self.advance();
                    if self.peek() == Some(b'=') {
                        self.advance();
                        tokens.push(Token::Lte);
                    } else {
                        tokens.push(Token::Lt);
                    }
                }
                b'>' => {
                    self.advance();
                    if self.peek() == Some(b'=') {
                        self.advance();
                        tokens.push(Token::Gte);
                    } else {
                        tokens.push(Token::Gt);
                    }
                }
                b'&' => {
                    self.advance();
                    if self.peek() == Some(b'&') {
                        self.advance();
                        tokens.push(Token::LogicalAnd);
                    } else {
                        return Err("Unexpected single '&'");
                    }
                }
                b'|' => {
                    self.advance();
                    if self.peek() == Some(b'|') {
                        self.advance();
                        tokens.push(Token::LogicalOr);
                    } else {
                        return Err("Unexpected single '|'");
                    }
                }
                b'"' | b'\'' | b'`' => {
                    let quote = self.advance().unwrap();
                    let mut s = String::new();
                    let mut closed = false;
                    while let Some(c) = self.advance() {
                        if c == quote {
                            closed = true;
                            break;
                        } else if c == b'\\' {
                            if let Some(escaped) = self.advance() {
                                match escaped {
                                    b'n' => s.push('\n'),
                                    b'r' => s.push('\r'),
                                    b't' => s.push('\t'),
                                    b'\\' => s.push('\\'),
                                    b'"' => s.push('"'),
                                    b'\'' => s.push('\''),
                                    b'`' => s.push('`'),
                                    other => s.push(other as char),
                                }
                            }
                        } else {
                            s.push(c as char);
                        }
                    }
                    if !closed {
                        return Err("Unterminated string literal");
                    }
                    tokens.push(Token::StringLit(s));
                }
                b'0'..=b'9' => {
                    let mut num = 0i64;
                    while let Some(d) = self.peek() {
                        if d.is_ascii_digit() {
                            num = num * 10 + (d - b'0') as i64;
                            self.advance();
                        } else {
                            break;
                        }
                    }
                    tokens.push(Token::Number(num));
                }
                b'a'..=b'z' | b'A'..=b'Z' | b'_' | b'$' => {
                    let mut id = String::new();
                    while let Some(c) = self.peek() {
                        if c.is_ascii_alphanumeric() || c == b'_' || c == b'$' {
                            id.push(c as char);
                            self.advance();
                        } else {
                            break;
                        }
                    }
                    match id.as_str() {
                        "const" => tokens.push(Token::Const),
                        "let" => tokens.push(Token::Let),
                        "var" => tokens.push(Token::Var),
                        "function" => tokens.push(Token::Function),
                        "return" => tokens.push(Token::Return),
                        "if" => tokens.push(Token::If),
                        "else" => tokens.push(Token::Else),
                        "while" => tokens.push(Token::While),
                        "for" => tokens.push(Token::For),
                        "await" => tokens.push(Token::Await),
                        "async" => tokens.push(Token::Async),
                        "true" => tokens.push(Token::True),
                        "false" => tokens.push(Token::False),
                        "null" => tokens.push(Token::Null),
                        _ => tokens.push(Token::Ident(id)),
                    }
                }
                _ => {
                    return Err("Unrecognized character in script");
                }
            }
        }

        Ok(tokens)
    }
}
