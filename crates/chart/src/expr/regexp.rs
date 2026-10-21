//! JavaScript regular expressions on `js::regex`.

use crate::expr::Error;
use js::regex::Regex;
use std::fmt;

/// A regular expression and the flags it was made with.
#[derive(Clone, Debug)]
pub struct RegExp {
    pub source: String,
    pub flags: String,
    pub regex: Regex,
}

impl RegExp {
    /// `new RegExp(source, flags)`, or the `SyntaxError` it throws.
    pub fn new(source: &str, flags: &str) -> Result<Self, Error> {
        let regex =
            Regex::try_new(source, flags).map_err(|error| Error::Runtime(error.to_string()))?;
        Ok(Self {
            source: source.to_string(),
            flags: flags.to_string(),
            regex,
        })
    }

    pub fn global(&self) -> bool {
        self.flags.contains('g')
    }
}

impl fmt::Display for RegExp {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let source = if self.source.is_empty() {
            "(?:)"
        } else {
            &self.source
        };
        write!(f, "/{source}/{}", self.flags)
    }
}
