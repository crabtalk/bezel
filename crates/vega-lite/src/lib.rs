//! [Vega-Lite](https://vega.github.io/vega-lite/) 6.5's compiler: a Vega-Lite
//! spec compiled to the Vega spec `vegaLite.compile(spec).spec` writes.
//!
//! A spec this does not read fails to parse into a [`spec::TopLevel`]; one
//! it reads but cannot compile answers [`Error::Unsupported`].

use std::fmt;

mod compile;
mod config;
pub mod spec;

pub use compile::compile;

#[derive(Clone, Debug, PartialEq)]
pub enum Error {
    /// What does not make a Vega spec.
    Spec(String),
    /// What Vega-Lite compiles and this does not.
    Unsupported(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Spec(message) => f.write_str(message),
            Self::Unsupported(what) => write!(f, "unsupported: {what}"),
        }
    }
}

impl std::error::Error for Error {}
