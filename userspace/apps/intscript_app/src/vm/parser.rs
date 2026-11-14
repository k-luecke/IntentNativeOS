// userspace/apps/script_app/src/vm/parser.rs - ECMAScript Recursive Descent Parser
extern crate alloc;
use alloc::boxed::Box;
use alloc::vec::Vec;
use super::ast::{BinOp, Expr, Statement};
use super::token::Token;

pub struct Parser {
    tokens: Vec<Token>,
    pos: usize,
}

impl Parser {
    pub fn new(tokens: Vec<Token>) -> Self {
        Self { tokens, pos: 0 }
    }

    fn peek(&self) -> &Token {
        if self.pos < self.tokens.len() {
            &self.tokens[self.pos]
        } else {
            &Token::Eof
        }
    }

    fn advance(&mut self) -> Token {
        if self.pos < self.tokens.len() {
            let tok = self.tokens[self.pos].clone();
            self.pos += 1;
            tok
        } else {
            Token::Eof
        }
    }

    fn expect(&mut self, expected: Token) -> Result<(), &'static str> {
        let tok = self.advance();
        if tok == expected {
            Ok(())
        } else {
            Err("Unexpected token encountered during parsing")
        }
    }

    pub fn parse_program(&mut self) -> Result<Vec<Statement>, &'static str> {
        let mut stmts = Vec::new();
        while self.peek() != &Token::Eof {
            stmts.push(self.parse_statement()?);
        }
        Ok(stmts)
    }

    fn parse_statement(&mut self) -> Result<Statement, &'static str> {
        match self.peek() {
            Token::Const | Token::Let | Token::Var => {
                let is_const = matches!(self.peek(), Token::Const);
                self.advance();
                let name = match self.advance() {
                    Token::Ident(id) => id,
                    _ => return Err("Expected identifier after variable declaration keyword"),
                };
                self.expect(Token::Assign)?;
                let init = self.parse_expr()?;
                if self.peek() == &Token::Semicolon {
                    self.advance();
                }
                Ok(Statement::VarDecl { name, init, is_const })
            }
            Token::If => {
                self.advance();
                self.expect(Token::LParen)?;
                let cond = self.parse_expr()?;
                self.expect(Token::RParen)?;
                let then_branch = self.parse_block_or_statement()?;
                let else_branch = if self.peek() == &Token::Else {
                    self.advance();
                    Some(self.parse_block_or_statement()?)
                } else {
                    None
                };
                Ok(Statement::IfStmt { cond, then_branch, else_branch })
            }
            Token::While => {
                self.advance();
                self.expect(Token::LParen)?;
                let cond = self.parse_expr()?;
                self.expect(Token::RParen)?;
                let body = self.parse_block_or_statement()?;
                Ok(Statement::WhileStmt { cond, body })
            }
            Token::Return => {
                self.advance();
                let ret_expr = if self.peek() != &Token::Semicolon && self.peek() != &Token::RBrace && self.peek() != &Token::Eof {
                    Some(self.parse_expr()?)
                } else {
                    None
                };
                if self.peek() == &Token::Semicolon {
                    self.advance();
                }
                Ok(Statement::ReturnStmt(ret_expr))
            }
            Token::LBrace => {
                let stmts = self.parse_block()?;
                Ok(Statement::Block(stmts))
            }
            _ => {
                let expr = self.parse_expr()?;
                if self.peek() == &Token::Semicolon {
                    self.advance();
                }
                Ok(Statement::ExprStmt(expr))
            }
        }
    }

    fn parse_block(&mut self) -> Result<Vec<Statement>, &'static str> {
        self.expect(Token::LBrace)?;
        let mut stmts = Vec::new();
        while self.peek() != &Token::RBrace && self.peek() != &Token::Eof {
            stmts.push(self.parse_statement()?);
        }
        self.expect(Token::RBrace)?;
        Ok(stmts)
    }

    fn parse_block_or_statement(&mut self) -> Result<Vec<Statement>, &'static str> {
        if self.peek() == &Token::LBrace {
            self.parse_block()
        } else {
            let stmt = self.parse_statement()?;
            let mut v = Vec::new();
            v.push(stmt);
            Ok(v)
        }
    }

    pub fn parse_expr(&mut self) -> Result<Expr, &'static str> {
        // Transparently skip `await` keyword in expressions
        if self.peek() == &Token::Await {
            self.advance();
        }
        self.parse_assignment()
    }

    fn parse_assignment(&mut self) -> Result<Expr, &'static str> {
        let left = self.parse_logical_or()?;
        if self.peek() == &Token::Assign {
            self.advance();
            let right = self.parse_expr()?;
            if let Expr::Variable(name) = left {
                return Ok(Expr::Assign(name, Box::new(right)));
            } else {
                return Err("Invalid assignment target");
            }
        }
        Ok(left)
    }

    fn parse_logical_or(&mut self) -> Result<Expr, &'static str> {
        let mut left = self.parse_logical_and()?;
        while self.peek() == &Token::LogicalOr {
            self.advance();
            let right = self.parse_logical_and()?;
            left = Expr::Binary(Box::new(left), BinOp::Or, Box::new(right));
        }
        Ok(left)
    }

    fn parse_logical_and(&mut self) -> Result<Expr, &'static str> {
        let mut left = self.parse_equality()?;
        while self.peek() == &Token::LogicalAnd {
            self.advance();
            let right = self.parse_equality()?;
            left = Expr::Binary(Box::new(left), BinOp::And, Box::new(right));
        }
        Ok(left)
    }

    fn parse_equality(&mut self) -> Result<Expr, &'static str> {
        let mut left = self.parse_relational()?;
        while matches!(self.peek(), Token::Eq | Token::Neq) {
            let op = match self.advance() {
                Token::Eq => BinOp::Eq,
                Token::Neq => BinOp::Neq,
                _ => unreachable!(),
            };
            let right = self.parse_relational()?;
            left = Expr::Binary(Box::new(left), op, Box::new(right));
        }
        Ok(left)
    }

    fn parse_relational(&mut self) -> Result<Expr, &'static str> {
        let mut left = self.parse_additive()?;
        while matches!(self.peek(), Token::Lt | Token::Gt | Token::Lte | Token::Gte) {
            let op = match self.advance() {
                Token::Lt => BinOp::Lt,
                Token::Gt => BinOp::Gt,
                Token::Lte => BinOp::Lte,
                Token::Gte => BinOp::Gte,
                _ => unreachable!(),
            };
            let right = self.parse_additive()?;
            left = Expr::Binary(Box::new(left), op, Box::new(right));
        }
        Ok(left)
    }

    fn parse_additive(&mut self) -> Result<Expr, &'static str> {
        let mut left = self.parse_multiplicative()?;
        while matches!(self.peek(), Token::Plus | Token::Minus) {
            let op = match self.advance() {
                Token::Plus => BinOp::Add,
                Token::Minus => BinOp::Sub,
                _ => unreachable!(),
            };
            let right = self.parse_multiplicative()?;
            left = Expr::Binary(Box::new(left), op, Box::new(right));
        }
        Ok(left)
    }

    fn parse_multiplicative(&mut self) -> Result<Expr, &'static str> {
        let mut left = self.parse_postfix()?;
        while matches!(self.peek(), Token::Star | Token::Slash) {
            let op = match self.advance() {
                Token::Star => BinOp::Mul,
                Token::Slash => BinOp::Div,
                _ => unreachable!(),
            };
            let right = self.parse_postfix()?;
            left = Expr::Binary(Box::new(left), op, Box::new(right));
        }
        Ok(left)
    }

    fn parse_postfix(&mut self) -> Result<Expr, &'static str> {
        let mut expr = self.parse_primary()?;

        loop {
            match self.peek() {
                Token::Dot => {
                    self.advance();
                    match self.advance() {
                        Token::Ident(prop) => {
                            expr = Expr::MemberAccess(Box::new(expr), prop);
                        }
                        _ => return Err("Expected property name after '.'"),
                    }
                }
                Token::LParen => {
                    self.advance();
                    let mut args = Vec::new();
                    if self.peek() != &Token::RParen {
                        loop {
                            args.push(self.parse_expr()?);
                            if self.peek() == &Token::Comma {
                                self.advance();
                            } else {
                                break;
                            }
                        }
                    }
                    self.expect(Token::RParen)?;
                    expr = Expr::Call(Box::new(expr), args);
                }
                _ => break,
            }
        }

        Ok(expr)
    }

    fn parse_primary(&mut self) -> Result<Expr, &'static str> {
        match self.advance() {
            Token::Number(n) => Ok(Expr::Number(n)),
            Token::StringLit(s) => Ok(Expr::Str(s)),
            Token::True => Ok(Expr::Bool(true)),
            Token::False => Ok(Expr::Bool(false)),
            Token::Null => Ok(Expr::Null),
            Token::Ident(id) => Ok(Expr::Variable(id)),
            Token::LParen => {
                let expr = self.parse_expr()?;
                self.expect(Token::RParen)?;
                Ok(expr)
            }
            Token::LBracket => {
                let mut elements = Vec::new();
                if self.peek() != &Token::RBracket {
                    loop {
                        elements.push(self.parse_expr()?);
                        if self.peek() == &Token::Comma {
                            self.advance();
                        } else {
                            break;
                        }
                    }
                }
                self.expect(Token::RBracket)?;
                Ok(Expr::Array(elements))
            }
            _ => Err("Unexpected token in primary expression"),
        }
    }
}
