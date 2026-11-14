// userspace/apps/script_app/src/vm/token.rs - ECMAScript Token Types
extern crate alloc;
use alloc::string::String;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Token {
    // Keywords
    Const,
    Let,
    Var,
    Function,
    Return,
    If,
    Else,
    While,
    For,
    Await,
    Async,
    True,
    False,
    Null,

    // Literals & Identifiers
    Ident(String),
    Number(i64),
    StringLit(String),

    // Operators & Punctuation
    Plus,
    Minus,
    Star,
    Slash,
    Assign,      // =
    Eq,          // ==
    Neq,         // !=
    Lt,          // <
    Gt,          // >
    Lte,         // <=
    Gte,         // >=
    Dot,         // .
    Comma,       // ,
    Colon,       // :
    Semicolon,   // ;
    LParen,      // (
    RParen,      // )
    LBrace,      // {
    RBrace,      // }
    LBracket,    // [
    RBracket,    // ]
    Arrow,       // =>
    LogicalAnd,  // &&
    LogicalOr,   // ||

    Eof,
}
