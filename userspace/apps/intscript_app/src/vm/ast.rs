// userspace/apps/script_app/src/vm/ast.rs - ECMAScript AST Nodes
extern crate alloc;
use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec::Vec;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Eq,
    Neq,
    Lt,
    Gt,
    Lte,
    Gte,
    And,
    Or,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Expr {
    Number(i64),
    Str(String),
    Bool(bool),
    Null,
    Variable(String),
    MemberAccess(Box<Expr>, String),
    Call(Box<Expr>, Vec<Expr>),
    Binary(Box<Expr>, BinOp, Box<Expr>),
    Assign(String, Box<Expr>),
    Array(Vec<Expr>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Statement {
    VarDecl {
        name: String,
        init: Expr,
        is_const: bool,
    },
    ExprStmt(Expr),
    IfStmt {
        cond: Expr,
        then_branch: Vec<Statement>,
        else_branch: Option<Vec<Statement>>,
    },
    WhileStmt {
        cond: Expr,
        body: Vec<Statement>,
    },
    ReturnStmt(Option<Expr>),
    Block(Vec<Statement>),
}
