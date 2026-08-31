//! The gallery's fenced-block renderer, which `markdown` asks for every fence.
//!
//! ```` ```chart ```` holds one `label: number` per line and paints as bars.
//! It is here to show the seam, not to be a chart library: a fence already
//! round trips and already holds a caret, so a block of an app's own is a
//! renderer rather than a new [`markdown::BlockKind`].
//!
//! Install with `markdown::set_block_renderer(cx, chart::render)`.

use gpui::{AnyElement, App, Window, div, prelude::*, px, relative};
use theme::{TextStyle, Theme, Typeset};

pub const LANGUAGE: &str = "chart";

pub fn render(language: &str, code: &str, _: &mut Window, cx: &mut App) -> Option<AnyElement> {
    if language != LANGUAGE {
        return None;
    }
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
            .gap(px(6.0))
            .p(px(12.0))
            .rounded(px(8.0))
            .bg(theme.ink(0.02))
            .children(rows.into_iter().map(|(label, value)| {
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(8.0))
                    .child(
                        div()
                            .flex_none()
                            .w(px(72.0))
                            .text_style(TextStyle::Caption)
                            .text_color(theme.text_muted)
                            .child(label.to_string()),
                    )
                    .child(
                        div().flex_1().child(
                            div()
                                .h(px(10.0))
                                .w(relative(value / peak))
                                .rounded(px(3.0))
                                .bg(theme.accent),
                        ),
                    )
            }))
            .into_any_element(),
    )
}
