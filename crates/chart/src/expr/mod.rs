//! [Vega expressions](https://vega.github.io/vega/docs/expressions/): the
//! JavaScript subset Vega accepts, evaluated with JavaScript's semantics over
//! one row of a [`Data`](crate::Data).
//!
//! `datum` is the row, the constants (`PI`, `E`, …) are numbers, and any other
//! bare name is a param. Calls name a built-in function; a name this does not
//! have fails to parse.
//!
//! Every time is milliseconds since the epoch. The date functions without
//! `utc` (`year`, `hours`, `datetime`, `timeOffset`, …) read and build dates
//! in the host's time zone, the `utc` ones in UTC, and a string reads as
//! `new Date(text)` reads it. A date is a number, so `isDate` is never true.
//!
//! Not here: `format`, `timeFormat`, `utcFormat`, `timeParse`, the statistical
//! functions, and every function that reads a runtime (scales, data sets,
//! selections, view state).

use crate::expr::parse::{Node, Parser};
pub use eval::Row;
pub use regexp::RegExp;
use std::{fmt, str::FromStr};
pub use value::Value;

mod array;
mod date;
mod eval;
mod function;
mod parse;
mod regexp;
mod string;
mod value;

/// A parsed expression.
#[derive(Clone, Debug)]
pub struct Expr(Node);

impl Expr {
    /// The expression's value on `row`, or the error JavaScript would throw.
    pub fn eval(&self, row: &Row) -> Result<Value, Error> {
        self.0.eval(row)
    }
}

impl FromStr for Expr {
    type Err = Error;

    fn from_str(source: &str) -> Result<Self, Error> {
        Parser::new(source).expression().map(Self)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Error {
    /// The source is not an expression. `at` is a byte offset into it.
    Syntax { at: usize, message: String },
    /// A call to a function this does not have.
    Function(String),
    /// What JavaScript would throw while evaluating.
    Runtime(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Syntax { at, message } => write!(f, "{message} at {at}"),
            Self::Function(name) => write!(f, "unknown function `{name}`"),
            Self::Runtime(message) => f.write_str(message),
        }
    }
}

impl std::error::Error for Error {}
