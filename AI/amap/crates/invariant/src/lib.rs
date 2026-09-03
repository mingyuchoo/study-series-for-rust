//! Business Invariant DSL: parser (pest) → AST → deterministic evaluator over JSON.
//!
//! Used for two things:
//! 1. Business invariants (`Debit Total = Credit Total`) that must hold for *both* legacy and next —
//!    catching cases where both systems are wrong (design §12).
//! 2. Business-rule conditions / results (`WHEN … THEN …`) so rules can be matched against
//!    production behaviors and checked against next-system outputs.

pub mod ast;
pub mod eval;
pub mod parser;

pub use ast::{BinOp, Expr, Func, Invariant, InvariantSet, UnOp};
pub use eval::{check, check_all, eval, eval_bool, InvariantOutcome, Val};
pub use parser::{parse, parse_expr};

#[derive(Debug, thiserror::Error)]
pub enum InvariantError {
    #[error("parse error: {0}")]
    Parse(String),
    #[error("type error: {0}")]
    Type(String),
    #[error("division by zero")]
    DivisionByZero,
}
