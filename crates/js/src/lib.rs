//! What JavaScript does with numbers, strings, regular expressions and
//! dates, where Rust's own answer differs and output would come out
//! different, and ports of d3-color, d3-format, d3-path, d3-shape's curves
//! and arc, d3-time and d3-time-format.
//!
//! Builds for `wasm32-unknown-unknown`, where the clock and the local zone
//! are the page's.

pub mod arc;
pub mod color;
pub mod curve;
pub mod date;
pub mod format;
pub mod math;
pub mod number;
pub mod path;
pub mod regex;
pub mod string;
pub mod time;
pub mod time_format;
