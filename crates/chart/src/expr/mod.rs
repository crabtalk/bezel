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
//! `format`, `timeFormat` and `timeParse` are d3's with the en-US locale,
//! `timeFormat` and `timeParse` in the host's zone, `utcFormat` and
//! `utcParse` in UTC. An invalid number format throws.
//!
//! Not here: the statistical functions, and every function that reads a
//! runtime (scales, data sets, selections, view state).

use crate::{
    expr::parse::{Node, Parser},
    value::{TypeError, Value, regexp::SyntaxError},
};
pub use eval::Row;
use std::{fmt, str::FromStr};

mod array;
mod date;
mod eval;
mod function;
mod parse;
mod string;

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

impl From<TypeError> for Error {
    fn from(error: TypeError) -> Self {
        Self::Runtime(error.0)
    }
}

/// `new RegExp`'s `SyntaxError`.
impl From<SyntaxError> for Error {
    fn from(error: SyntaxError) -> Self {
        Self::Runtime(error.to_string())
    }
}
