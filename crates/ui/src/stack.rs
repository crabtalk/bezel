//! The two stacks, at the system gap.
//!
//! SwiftUI's `HStack` and `VStack`: spacing is what you get by *not* asking for
//! it, and a caller who needs another value chains `.gap(..)` the way
//! `VStack(spacing: 12)` says it. The number leaves the call site rather than
//! taking a name.

use gpui::{Div, div, prelude::*, px};
use theme::Theme;

/// A row at [`Theme::SPACE`], centred across — SwiftUI's `HStack`, whose
/// default alignment is `.center`.
pub fn row() -> Div {
    div().flex().flex_row().items_center().gap(px(Theme::SPACE))
}

/// A column at [`Theme::SPACE`]. SwiftUI centres a `VStack`'s children across;
/// this does not, because flexbox stretches them and every column in the crate
/// is built on that.
pub fn column() -> Div {
    div().flex().flex_col().gap(px(Theme::SPACE))
}
