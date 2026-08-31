//! ` ```chart ` — one `label: number` per line, painted as bars.
//!
//! Deliberately the smallest block worth shipping: no axes, no scales, no
//! legend. What it demonstrates is the shape of a block, and a chart library
//! behind a fence tag is a different crate's job.

use gpui::{AnyElement, App, Window, div, prelude::*, px, relative};
use theme::{TextStyle, Theme, Typeset};

pub const LANGUAGE: &str = "chart";

/// The label column. Wide enough for a word, narrow enough that the bars still
/// carry the row.
const LABEL_WIDTH: f32 = 72.0;
const BAR_HEIGHT: f32 = 10.0;
const BAR_RADIUS: f32 = 3.0;
const ROW_GAP: f32 = 4.0;
const PADDING: f32 = 12.0;

pub fn render(code: &str, _: &mut Window, cx: &mut App) -> Option<AnyElement> {
    let rows: Vec<(&str, f32)> = code
        .lines()
        .filter_map(|line| {
            let (label, value) = line.split_once(':')?;
            Some((label.trim(), value.trim().parse().ok()?))
        })
        .collect();
    // A fence holding nothing a chart can read is one still being typed, and
    // the source is more use than an empty box.
    if rows.is_empty() {
        return None;
    }

    let theme = Theme::of(cx);
    let peak = rows.iter().map(|(_, value)| *value).fold(1.0, f32::max);
    Some(
        div()
            .flex()
            .flex_col()
            .gap(px(ROW_GAP))
            .p(px(PADDING))
            .rounded(px(Theme::BASE_RADIUS))
            .bg(theme.ink(0.02))
            .children(rows.into_iter().map(|(label, value)| {
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(Theme::SPACE))
                    .child(
                        div()
                            .flex_none()
                            .w(px(LABEL_WIDTH))
                            .text_style(TextStyle::Caption)
                            .text_color(theme.text_muted)
                            .child(label.to_string()),
                    )
                    .child(
                        div().flex_1().child(
                            div()
                                .h(px(BAR_HEIGHT))
                                .w(relative(value / peak))
                                .rounded(px(BAR_RADIUS))
                                .bg(theme.accent),
                        ),
                    )
            }))
            .into_any_element(),
    )
}
