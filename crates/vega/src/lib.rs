//! [Vega](https://vega.github.io/vega/) 6.4's runtime: a Vega spec run to
//! the scenegraph Vega draws from it, as `view.scenegraph()` holds it.
//! Text widths are Vega's estimate, as Vega measures without a canvas.
//!
//! A spec this does not read fails to parse into a [`spec::Spec`]; one it
//! reads but cannot run answers [`Error::Unsupported`].

#![recursion_limit = "256"]

use std::fmt;

mod bound;
mod config;
mod encode;
pub mod expr;
mod guide;
mod layout;
mod loader;
pub mod path;
pub mod scale;
pub mod scene;
pub mod spec;
pub mod statistics;
pub mod text;
pub mod time;
pub mod transform;
pub mod value;
mod view;

pub use view::{Drawing, Loader, View};

#[derive(Clone, Debug, PartialEq)]
pub enum Error {
    /// What Vega's parser rejects.
    Spec(String),
    /// An expression that does not parse.
    Expression(String, expr::Error),
    /// What an expression throws.
    Eval(expr::Error),
    /// A url the loader could not read.
    Load(String),
    /// What Vega runs and this does not.
    Unsupported(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Spec(message) | Self::Load(message) => f.write_str(message),
            Self::Expression(code, error) => write!(f, "{error} in `{code}`"),
            Self::Eval(error) => write!(f, "{error}"),
            Self::Unsupported(what) => write!(f, "unsupported: {what}"),
        }
    }
}

impl std::error::Error for Error {}
