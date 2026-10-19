//! Text: entity codes, how wide a run is, and labels broken into lines.

mod entities;
mod icons;
mod label;
mod measure;
mod metrics;

pub use entities::decode_entities;
pub use icons::{icon, icon_markup};
pub use label::{Icon, LINE_HEIGHT, Label, Span, layout_label};
pub use measure::{Host, Metrics, Style};
