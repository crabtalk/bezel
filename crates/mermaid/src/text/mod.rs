//! Text: entity codes, how wide a run is, and labels broken into lines.

mod entities;
mod icons;
mod label;
mod measure;
mod metrics;

pub use entities::{decode_entities, decoded};
pub use icons::{has_icon, icon, icon_markup};
pub use label::{Icon, LINE_HEIGHT, Label, Span};
pub use measure::{Host, Metrics, Style};
