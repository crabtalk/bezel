//! Charts for gpui: a [`Chart`] is a subset of
//! [Vega-Lite](https://vega.github.io/vega-lite/) over columnar [`Data`].
//! Mark, channel and field type names are Vega-Lite's.

pub mod data;
pub mod decimate;
#[cfg(feature = "mermaid")]
pub mod mermaid;
pub mod model;
pub mod plan;
pub mod scale;
pub mod time;
#[cfg(feature = "vega-lite")]
pub mod vega_lite;
pub mod view;

pub use data::{Column, Data, Text};
pub use model::{Channel, Chart, Encoding, Kind, Mark, Scale, Sort};
pub use plan::{Plan, plan};
pub use view::{ChartView, view};
