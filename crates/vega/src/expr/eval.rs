//! A syntax tree's value in a context.

use crate::{
    expr::{
        Error,
        parse::{Binary, Node, Unary},
    },
    scale::Scale,
    value::Value,
};
use std::cmp::Ordering;

/// What an expression reads: `datum`, a bare name through `param` (a
/// signal, or `item` in an encoder), and a scale by name through `scale`.
#[derive(Clone, Copy)]
pub struct Context<'a, 's> {
    pub datum: &'a Value,
    pub param: &'a dyn Fn(&str) -> Value,
    pub scale: &'a dyn Fn(&str) -> Option<&'s Scale>,
}

impl Node {
    pub(crate) fn eval(&self, row: &Context<'_, '_>) -> Result<Value, Error> {
        Ok(match self {
            Self::Literal(value) => value.clone(),
            Self::Array(items) => Value::Array(
                items
                    .iter()
                    .map(|item| item.eval(row))
                    .collect::<Result<_, _>>()?,
            ),
            Self::Object(fields) => Value::Object(
                fields
                    .iter()
                    .map(|(key, node)| Ok((key.clone(), node.eval(row)?)))
                    .collect::<Result<_, Error>>()?,
            ),
            Self::Datum => row.datum.clone(),
            Self::Param(name) => (row.param)(name),
            Self::Member(object, property) => {
                let key = property.eval(row)?;
                match **object {
                    Self::Datum => row.datum.get(&key)?,
                    _ => object.eval(row)?.get(&key)?,
                }
            }
            Self::Unary(op, operand) => {
                let value = operand.eval(row)?;
                match op {
                    Unary::Not => Value::Bool(!value.truthy()),
                    Unary::Negate => Value::Number(-value.to_number()),
                    Unary::Plus => Value::Number(value.to_number()),
                    Unary::BitNot => Value::Number(f64::from(!value.to_int32())),
                }
            }
            Self::Binary(op, left, right) => op.apply(&left.eval(row)?, &right.eval(row)?),
            Self::And(left, right) => {
                let left = left.eval(row)?;
                if left.truthy() {
                    right.eval(row)?
                } else {
                    left
                }
            }
            Self::Or(left, right) => {
                let left = left.eval(row)?;
                if left.truthy() {
                    left
                } else {
                    right.eval(row)?
                }
            }
            Self::Conditional(test, then, otherwise) => {
                if test.eval(row)?.truthy() {
                    then.eval(row)?
                } else {
                    otherwise.eval(row)?
                }
            }
            Self::Call(function, args) => function.call(args, row)?,
        })
    }
}

impl Binary {
    fn apply(self, a: &Value, b: &Value) -> Value {
        let number = |f: fn(f64, f64) -> f64| Value::Number(f(a.to_number(), b.to_number()));
        let int = |f: fn(i32, i32) -> i32| Value::Number(f64::from(f(a.to_int32(), b.to_int32())));
        let shift = b.to_uint32() & 31;
        match self {
            Self::Add => a + b,
            Self::Sub => number(|x, y| x - y),
            Self::Mul => number(|x, y| x * y),
            Self::Div => number(|x, y| x / y),
            Self::Rem => number(|x, y| x % y),
            Self::Shl => Value::Number(f64::from(a.to_int32().wrapping_shl(shift))),
            Self::Shr => Value::Number(f64::from(a.to_int32() >> shift)),
            Self::UShr => Value::Number(f64::from(a.to_uint32() >> shift)),
            Self::BitAnd => int(|x, y| x & y),
            Self::BitOr => int(|x, y| x | y),
            Self::BitXor => int(|x, y| x ^ y),
            Self::Eq => Value::Bool(a.loose_eq(b)),
            Self::Ne => Value::Bool(!a.loose_eq(b)),
            Self::StrictEq => Value::Bool(a.strict_eq(b)),
            Self::StrictNe => Value::Bool(!a.strict_eq(b)),
            Self::Lt => Value::Bool(a.compare(b) == Some(Ordering::Less)),
            Self::Gt => Value::Bool(a.compare(b) == Some(Ordering::Greater)),
            Self::Le => Value::Bool(matches!(
                a.compare(b),
                Some(Ordering::Less | Ordering::Equal)
            )),
            Self::Ge => Value::Bool(matches!(
                a.compare(b),
                Some(Ordering::Greater | Ordering::Equal)
            )),
        }
    }
}
