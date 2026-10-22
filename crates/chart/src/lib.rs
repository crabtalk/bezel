//! Charts for gpui: a [`Chart`] is a subset of
//! [Vega-Lite](https://vega.github.io/vega-lite/) over columnar [`Data`].
//! Mark, channel and field type names are Vega-Lite's.

mod color;
pub mod data;
pub mod decimate;
#[cfg(feature = "vega-lite")]
pub mod expr;
pub mod model;
mod path;
pub mod plan;
pub mod scale;
pub mod time;
pub mod transform;
pub mod value;
#[cfg(feature = "vega-lite")]
pub mod vega_lite;
pub mod view;

pub use data::{Column, Data, Name, Text};
pub use model::{
    Channel, Chart, Datum, Encode, Encoding, Kind, Layer, Mark, Position, Properties, Scale, Shape,
    Sort, Source,
};
pub use plan::{Plan, plan};
pub use transform::Transform;
pub use view::{ChartView, view};
