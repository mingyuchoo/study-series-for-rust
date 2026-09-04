use crate::ast::*;
use crate::InvariantError;
use serde::{Deserialize, Serialize};
use serde_json::Value;

const EPS: f64 = 1e-9;

/// Runtime value of the DSL.
#[derive(Clone, Debug, PartialEq)]
pub enum Val {
    Null,
    Bool(bool),
    Num(f64),
    Str(String),
    List(Vec<Val>),
}

impl Val {
    pub fn truthy(&self) -> bool {
        match self {
            Val::Null => false,
            Val::Bool(b) => *b,
            Val::Num(n) => *n != 0.0,
            Val::Str(s) => !s.is_empty(),
            Val::List(l) => !l.is_empty(),
        }
    }
    fn as_num(&self) -> Result<f64, InvariantError> {
        match self {
            Val::Num(n) => Ok(*n),
            Val::Bool(b) => Ok(if *b { 1.0 } else { 0.0 }),
            Val::Str(s) => s
                .trim()
                .parse()
                .map_err(|_| InvariantError::Type(format!("`{s}` is not numeric"))),
            Val::Null => Ok(0.0),
            Val::List(_) => Err(InvariantError::Type("list used as number".into())),
        }
    }
    fn flatten_nums(&self) -> Result<Vec<f64>, InvariantError> {
        match self {
            Val::List(l) => {
                let mut out = Vec::new();
                for v in l {
                    out.extend(v.flatten_nums()?);
                }
                Ok(out)
            }
            Val::Null => Ok(vec![]),
            other => Ok(vec![other.as_num()?]),
        }
    }
}

fn from_json(v: &Value) -> Val {
    match v {
        Value::Null => Val::Null,
        Value::Bool(b) => Val::Bool(*b),
        Value::Number(n) => Val::Num(n.as_f64().unwrap_or(0.0)),
        Value::String(s) => Val::Str(s.clone()),
        Value::Array(a) => Val::List(a.iter().map(from_json).collect()),
        Value::Object(_) => Val::Null,
    }
}

/// Resolve a dotted path; arrays are mapped over (`debit.amount` → list of amounts).
fn resolve(path: &[String], v: &Value) -> Val {
    if path.is_empty() {
        return from_json(v);
    }
    match v {
        Value::Object(m) => match m.get(&path[0]) {
            Some(child) => resolve(&path[1..], child),
            None => Val::Null,
        },
        Value::Array(a) => Val::List(a.iter().map(|e| resolve(path, e)).collect()),
        _ => Val::Null,
    }
}

pub fn eval(expr: &Expr, data: &Value) -> Result<Val, InvariantError> {
    Ok(match expr {
        Expr::Null => Val::Null,
        Expr::Bool(b) => Val::Bool(*b),
        Expr::Num(n) => Val::Num(*n),
        Expr::Str(s) => Val::Str(s.clone()),
        Expr::Path(p) => resolve(p, data),
        Expr::Unary(UnOp::Not, e) => Val::Bool(!eval(e, data)?.truthy()),
        Expr::Unary(UnOp::Neg, e) => Val::Num(-eval(e, data)?.as_num()?),
        Expr::Binary(op, l, r) => {
            match op {
                BinOp::And => {
                    let lv = eval(l, data)?;
                    return Ok(Val::Bool(lv.truthy() && eval(r, data)?.truthy()));
                }
                BinOp::Or => {
                    let lv = eval(l, data)?;
                    return Ok(Val::Bool(lv.truthy() || eval(r, data)?.truthy()));
                }
                _ => {}
            }
            let lv = eval(l, data)?;
            let rv = eval(r, data)?;
            match op {
                BinOp::Eq => Val::Bool(equal(&lv, &rv)),
                BinOp::Ne => Val::Bool(!equal(&lv, &rv)),
                BinOp::Lt => Val::Bool(cmp(&lv, &rv)? < 0),
                BinOp::Le => Val::Bool(cmp(&lv, &rv)? <= 0),
                BinOp::Gt => Val::Bool(cmp(&lv, &rv)? > 0),
                BinOp::Ge => Val::Bool(cmp(&lv, &rv)? >= 0),
                BinOp::Add => Val::Num(lv.as_num()? + rv.as_num()?),
                BinOp::Sub => Val::Num(lv.as_num()? - rv.as_num()?),
                BinOp::Mul => Val::Num(lv.as_num()? * rv.as_num()?),
                BinOp::Div => {
                    let d = rv.as_num()?;
                    if d == 0.0 {
                        return Err(InvariantError::DivisionByZero);
                    }
                    Val::Num(lv.as_num()? / d)
                }
                BinOp::Mod => {
                    let d = rv.as_num()?;
                    if d == 0.0 {
                        return Err(InvariantError::DivisionByZero);
                    }
                    Val::Num(lv.as_num()? % d)
                }
                BinOp::And | BinOp::Or => unreachable!(),
            }
        }
        Expr::Call(func, args) => {
            let vals = args
                .iter()
                .map(|a| eval(a, data))
                .collect::<Result<Vec<_>, _>>()?;
            let first = vals.first().cloned().unwrap_or(Val::Null);
            match func {
                Func::Sum => Val::Num(first.flatten_nums()?.iter().sum()),
                Func::Count | Func::Len => Val::Num(match &first {
                    Val::List(l) => l.len() as f64,
                    Val::Null => 0.0,
                    Val::Str(s) => s.len() as f64,
                    _ => 1.0,
                }),
                Func::Min => Val::Num(all_nums(&vals)?.into_iter().fold(f64::INFINITY, f64::min)),
                Func::Max => Val::Num(
                    all_nums(&vals)?
                        .into_iter()
                        .fold(f64::NEG_INFINITY, f64::max),
                ),
                Func::Abs => Val::Num(first.as_num()?.abs()),
                Func::Round => {
                    let places = vals.get(1).map(|v| v.as_num()).transpose()?.unwrap_or(0.0);
                    let f = 10f64.powi(places as i32);
                    Val::Num((first.as_num()? * f).round() / f)
                }
                Func::Exists => Val::Bool(!matches!(first, Val::Null)),
                Func::All => Val::Bool(match &first {
                    Val::List(l) => l.iter().all(Val::truthy),
                    v => v.truthy(),
                }),
                Func::Any => Val::Bool(match &first {
                    Val::List(l) => l.iter().any(Val::truthy),
                    v => v.truthy(),
                }),
            }
        }
    })
}

fn all_nums(vals: &[Val]) -> Result<Vec<f64>, InvariantError> {
    let mut out = Vec::new();
    for v in vals {
        out.extend(v.flatten_nums()?);
    }
    Ok(out)
}

/// Relative-epsilon float equality (financial magnitudes up to 1e15 stay exact at the won).
fn feq(x: f64, y: f64) -> bool {
    (x - y).abs() <= EPS * x.abs().max(y.abs()).max(1.0)
}

fn equal(a: &Val, b: &Val) -> bool {
    match (a, b) {
        (Val::Num(x), Val::Num(y)) => feq(*x, *y),
        (Val::Num(_), Val::Str(_)) | (Val::Str(_), Val::Num(_)) => match (a.as_num(), b.as_num()) {
            (Ok(x), Ok(y)) => feq(x, y),
            _ => false,
        },
        (Val::List(x), Val::List(y)) => {
            x.len() == y.len() && x.iter().zip(y).all(|(p, q)| equal(p, q))
        }
        _ => a == b,
    }
}

fn cmp(a: &Val, b: &Val) -> Result<i8, InvariantError> {
    match (a, b) {
        (Val::Str(x), Val::Str(y)) => Ok(x.cmp(y) as i8),
        _ => {
            let (x, y) = (a.as_num()?, b.as_num()?);
            Ok(if feq(x, y) {
                0
            } else if x < y {
                -1
            } else {
                1
            })
        }
    }
}

pub fn eval_bool(expr: &Expr, data: &Value) -> Result<bool, InvariantError> {
    Ok(eval(expr, data)?.truthy())
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct InvariantOutcome {
    pub name: String,
    /// Guard evaluated to true (or no guard).
    pub applicable: bool,
    pub holds: bool,
    pub detail: String,
}

pub fn check(inv: &Invariant, data: &Value) -> InvariantOutcome {
    let applicable = match &inv.guard {
        Some(g) => eval_bool(g, data).unwrap_or(false),
        None => true,
    };
    if !applicable {
        return InvariantOutcome {
            name: inv.name.clone(),
            applicable,
            holds: true,
            detail: "guard not satisfied".into(),
        };
    }
    match eval_bool(&inv.body, data) {
        Ok(true) => InvariantOutcome {
            name: inv.name.clone(),
            applicable,
            holds: true,
            detail: "holds".into(),
        },
        Ok(false) => InvariantOutcome {
            name: inv.name.clone(),
            applicable,
            holds: false,
            detail: describe(&inv.body, data),
        },
        Err(e) => InvariantOutcome {
            name: inv.name.clone(),
            applicable,
            holds: false,
            detail: format!("evaluation error: {e}"),
        },
    }
}

pub fn check_all(set: &InvariantSet, data: &Value) -> Vec<InvariantOutcome> {
    set.rules.iter().map(|r| check(r, data)).collect()
}

fn describe(body: &Expr, data: &Value) -> String {
    if let Expr::Binary(op, l, r) = body {
        if let (Ok(lv), Ok(rv)) = (eval(l, data), eval(r, data)) {
            return format!("{lv:?} {op:?} {rv:?} is false");
        }
    }
    "violated".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::{parse, parse_expr};
    use serde_json::json;

    #[test]
    fn accounting_invariant() {
        let set = parse("RULE ACCOUNTING-001\nSUM(debit.amount) == SUM(credit.amount)").unwrap();
        let ok = json!({"debit": [{"amount": 10}, {"amount": 5}], "credit": [{"amount": 15}]});
        let bad = json!({"debit": [{"amount": 10}], "credit": [{"amount": 15}]});
        assert!(check_all(&set, &ok)[0].holds);
        assert!(!check_all(&set, &bad)[0].holds);
    }

    #[test]
    fn guarded_rule_and_arith() {
        let set = parse(
            r#"
            RULE BR-LOAN-000183 WHEN customer.grade == "VIP" AND loan.age_years > 3 AND repayment.type == "EARLY" THEN fee == 0
            RULE LOAN-002 previous_balance + deposits - withdrawals == current_balance
            "#,
        )
        .unwrap();
        let data = json!({"customer": {"grade": "VIP"}, "loan": {"age_years": 4}, "repayment": {"type": "EARLY"}, "fee": 0,
                          "previous_balance": 100, "deposits": 20, "withdrawals": 30, "current_balance": 90});
        let out = check_all(&set, &data);
        assert!(out.iter().all(|o| o.holds && o.applicable));
        let na = json!({"customer": {"grade": "GOLD"}, "fee": 5, "previous_balance": 1, "deposits": 0, "withdrawals": 0, "current_balance": 1});
        let out = check_all(&set, &na);
        assert!(!out[0].applicable && out[0].holds);
    }

    #[test]
    fn expression_eval() {
        let e = parse_expr("ROUND(principal * rate / 365, 2) >= 0 AND NOT (x == null)").unwrap();
        assert!(eval_bool(&e, &json!({"principal": 1000, "rate": 0.05, "x": 1})).unwrap());
    }
}
