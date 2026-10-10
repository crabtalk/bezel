//! Charts for gpui: a [`Chart`] is a subset of
//! [Vega-Lite](https://vega.github.io/vega-lite/) over columnar [`Data`].
//! Mark, channel and field type names are Vega-Lite's.

pub mod data;
pub mod model;

pub use data::{Column, Data, Text};
pub use model::{Channel, Chart, Encoding, Kind, Mark};
