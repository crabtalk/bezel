//! Ports of d3-array's ticks, d3-color, d3-dsv, d3-format, d3-interpolate,
//! d3-path, d3-scale, d3-shape's curves and arc, d3-time, d3-time-format,
//! and vega-scale's schemes, with JavaScript's numbers, strings, `Date` and
//! `Math` where Rust's own answer differs and output would come out
//! different.
//!
//! Builds for `wasm32-unknown-unknown`, where the clock and the local zone
//! are the page's.

pub mod array;
pub mod color;
pub mod dsv;
pub mod format;
pub mod interpolate;
pub mod math;
pub mod scale;
pub mod scheme;
pub mod shape;
pub mod string;
pub mod time;
pub mod time_format;
