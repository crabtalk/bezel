//! d3-scale 4.0.2's scales, with Vega's band and point and its
//! `bin-ordinal`: each maps a domain value to a number, a place in its
//! range, or an index into a range of any kind.

mod band;
mod continuous;
mod discretizing;
mod sequential;

pub use band::Band;
pub use continuous::{Continuous, Transform};
pub use discretizing::{BinOrdinal, Quantile, Quantize, Threshold};
pub use sequential::{Diverging, Sequential};
