use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Expr {
    Null,
    Bool(bool),
    Num(f64),
    Str(String),
    /// Dotted JSON path; traversing an array maps over its elements.
    Path(Vec<String>),
    Unary(UnOp, Box<Expr>),
    Binary(BinOp, Box<Expr>, Box<Expr>),
    Call(Func, Vec<Expr>),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum UnOp {
    Not,
    Neg,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum BinOp {
    Or,
    And,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    Add,
    Sub,
    Mul,
    Div,
    Mod,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Func {
    Sum,
    Count,
    Min,
    Max,
    Abs,
    Round,
    Exists,
    Len,
    All,
    Any,
}

/// `RULE name [WHEN guard THEN] body`
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Invariant {
    pub name: String,
    pub guard: Option<Expr>,
    pub body: Expr,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct InvariantSet {
    pub rules: Vec<Invariant>,
}
