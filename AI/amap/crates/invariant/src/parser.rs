use crate::ast::*;
use crate::InvariantError;
use pest::iterators::Pair;
use pest::Parser;
use pest_derive::Parser;

#[derive(Parser)]
#[grammar = "invariant.pest"]
struct DslParser;

pub fn parse(text: &str) -> Result<InvariantSet, InvariantError> {
    let mut pairs = DslParser::parse(Rule::file, text).map_err(|e| InvariantError::Parse(e.to_string()))?;
    let file = pairs.next().unwrap();
    let mut rules = Vec::new();
    for p in file.into_inner() {
        if p.as_rule() == Rule::rule {
            rules.push(build_rule(p)?);
        }
    }
    Ok(InvariantSet { rules })
}

pub fn parse_expr(text: &str) -> Result<Expr, InvariantError> {
    let mut pairs = DslParser::parse(Rule::single_expr, text).map_err(|e| InvariantError::Parse(e.to_string()))?;
    let single = pairs.next().unwrap();
    let expr = single.into_inner().find(|p| p.as_rule() == Rule::expr).unwrap();
    build_expr(expr)
}

fn build_rule(p: Pair<Rule>) -> Result<Invariant, InvariantError> {
    let mut name = String::new();
    let mut exprs = Vec::new();
    for inner in p.into_inner() {
        match inner.as_rule() {
            Rule::rule_name => name = inner.as_str().to_string(),
            Rule::expr => exprs.push(build_expr(inner)?),
            _ => {}
        }
    }
    let body = exprs.pop().ok_or_else(|| InvariantError::Parse(format!("rule {name} has no body")))?;
    let guard = exprs.pop();
    Ok(Invariant { name, guard, body })
}

fn build_expr(p: Pair<Rule>) -> Result<Expr, InvariantError> {
    match p.as_rule() {
        Rule::expr => build_expr(p.into_inner().next().unwrap()),
        Rule::or_expr => fold_binary(p, |op| match op {
            "or" => BinOp::Or,
            _ => BinOp::Or,
        }),
        Rule::and_expr => fold_binary(p, |_| BinOp::And),
        Rule::not_expr => {
            let mut inner = p.into_inner();
            let first = inner.next().unwrap();
            if first.as_rule() == Rule::not_op {
                Ok(Expr::Unary(UnOp::Not, Box::new(build_expr(inner.next().unwrap())?)))
            } else {
                build_expr(first)
            }
        }
        Rule::comparison => fold_binary(p, |op| match op {
            "==" | "=" => BinOp::Eq,
            "!=" => BinOp::Ne,
            "<" => BinOp::Lt,
            "<=" => BinOp::Le,
            ">" => BinOp::Gt,
            _ => BinOp::Ge,
        }),
        Rule::arith => fold_binary(p, |op| if op == "+" { BinOp::Add } else { BinOp::Sub }),
        Rule::term => fold_binary(p, |op| match op {
            "*" => BinOp::Mul,
            "/" => BinOp::Div,
            _ => BinOp::Mod,
        }),
        Rule::factor => build_expr(p.into_inner().next().unwrap()),
        Rule::neg => Ok(Expr::Unary(UnOp::Neg, Box::new(build_expr(p.into_inner().next().unwrap())?))),
        Rule::func_call => {
            let mut inner = p.into_inner();
            let name = inner.next().unwrap().as_str().to_ascii_uppercase();
            let func = match name.as_str() {
                "SUM" => Func::Sum,
                "COUNT" => Func::Count,
                "MIN" => Func::Min,
                "MAX" => Func::Max,
                "ABS" => Func::Abs,
                "ROUND" => Func::Round,
                "EXISTS" => Func::Exists,
                "LEN" => Func::Len,
                "ALL" => Func::All,
                "ANY" => Func::Any,
                other => return Err(InvariantError::Parse(format!("unknown function {other}"))),
            };
            let args = inner.map(build_expr).collect::<Result<Vec<_>, _>>()?;
            Ok(Expr::Call(func, args))
        }
        Rule::number => p.as_str().parse::<f64>().map(Expr::Num).map_err(|e| InvariantError::Parse(e.to_string())),
        Rule::string => {
            let s = p.as_str();
            Ok(Expr::Str(s[1..s.len() - 1].to_string()))
        }
        Rule::boolean => Ok(Expr::Bool(p.as_str().eq_ignore_ascii_case("true"))),
        Rule::null => Ok(Expr::Null),
        Rule::path => Ok(Expr::Path(p.as_str().split('.').map(|s| s.to_string()).collect())),
        other => Err(InvariantError::Parse(format!("unexpected node {other:?}"))),
    }
}

fn fold_binary(p: Pair<Rule>, op_of: impl Fn(&str) -> BinOp) -> Result<Expr, InvariantError> {
    let mut inner = p.into_inner();
    let mut lhs = build_expr(inner.next().unwrap())?;
    while let Some(op) = inner.next() {
        let rhs = build_expr(inner.next().unwrap())?;
        lhs = Expr::Binary(op_of(&op.as_str().to_ascii_lowercase()), Box::new(lhs), Box::new(rhs));
    }
    Ok(lhs)
}
