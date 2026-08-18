//! A file review — a diff with a header over it, which is what an agent shows
//! after it edits something.
//!
//! **This produced no library code, and the attempt is the finding.** A diff
//! row is two numbers, a sign and a line of text: three `div`s and a colour,
//! with no reducer, no state and nothing to measure. Next to `tree` (flattening
//! plus arrow-key walking) or `table` (a sort reducer and a cell-count guard)
//! it would be a paint helper wearing a component's badge, so it lives here
//! instead, in the file you would copy.
//!
//! What *is* hard about a diff view — folding hunks, word-level marks inside a
//! changed line, syntax, two panes scrolling together — is either the app's or
//! waits on a syntax crate. None of it is here, and calling this a component
//! would have promised all four.
//!
//! bezel never computes a diff. These rows arrive already decided; whatever
//! produced them is the app's business, the same line `tree` holds.

use bezel_theme::{Theme, ink};
use bezel_ui::{icons, widgets::Scaffolding};
use gpui::{Context, Render, SharedString, Window, div, prelude::*, px};

/// What happened to a line. `Skip` is the gap between hunks — the lines nobody
/// asked to see.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Change {
    Added,
    Removed,
    Context,
    Skip,
}

/// `(change, old line, new line, text)`.
type Row = (Change, Option<u32>, Option<u32>, &'static str);

const FILE: &str = "crates/ui/src/scroll.rs";

const ROWS: &[Row] = &[
    (Change::Skip, None, None, "@@ -232,7 +232,12 @@"),
    (
        Change::Context,
        Some(232),
        Some(232),
        "pub fn at_bottom(max_offset: Pixels, offset: Pixels) -> bool {",
    ),
    (
        Change::Removed,
        Some(233),
        None,
        "    max_offset - offset.abs() <= px(0.0)",
    ),
    (
        Change::Added,
        None,
        Some(233),
        "    // Content that fits is always at the bottom: there is nowhere",
    ),
    (
        Change::Added,
        None,
        Some(234),
        "    // else to be, and false would unpin a log that can never re-pin.",
    ),
    (
        Change::Added,
        None,
        Some(235),
        "    if max_offset <= px(0.0) {",
    ),
    (Change::Added, None, Some(236), "        return true;"),
    (Change::Added, None, Some(237), "    }"),
    (
        Change::Added,
        None,
        Some(238),
        "    max_offset - offset.clamp(-max_offset, px(0.0)).abs() <= slack",
    ),
    (Change::Context, Some(234), Some(239), "}"),
    (Change::Skip, None, None, "@@ -410,3 +415,3 @@"),
];

/// Width of a line-number column: five monospace figures, so a file of 99,999
/// lines still lines up and nothing reflows partway down.
const NUMBERS: f32 = 30.0;
/// Width of the `+`/`-` column.
const SIGN: f32 = 12.0;
/// How strongly a changed row tints. Low: a block of added lines is a lot of
/// surface, and the sign column is what you actually read.
const WASH: f32 = 0.10;

#[derive(Default)]
pub struct Diff;

impl Diff {
    /// One line. `old`/`new` are `None` where that side has no line — an added
    /// row has no old number — so the columns stay aligned without inventing a
    /// placeholder.
    fn row(theme: &Theme, (change, old, new, text): Row) -> gpui::Div {
        let (wash, mark, sign) = match change {
            Change::Added => (theme.success.opacity(WASH), theme.success, "+"),
            Change::Removed => (theme.danger.opacity(WASH), theme.danger, "-"),
            Change::Context | Change::Skip => (gpui::transparent_black(), theme.text_faint, " "),
        };
        let number = |value: Option<u32>| {
            div()
                .w(px(NUMBERS))
                .flex_none()
                .text_align(gpui::TextAlign::Right)
                .text_color(theme.text_faint)
                .child(match value {
                    Some(value) => SharedString::from(value.to_string()),
                    None => SharedString::default(),
                })
        };

        let line = div()
            .flex()
            .flex_row()
            .items_start()
            .gap(px(8.0))
            .px(px(10.0))
            .py(px(1.0))
            .bg(wash)
            .font_family(theme.font_mono.clone())
            .text_size(px(12.0))
            .line_height(px(18.0));

        if change == Change::Skip {
            // No numbers and no sign: the gap stands for however many lines
            // were left out, so there is no one line to name.
            return line
                .bg(ink(0.02))
                .child(
                    div()
                        .w(px(NUMBERS * 2.0 + SIGN + 16.0))
                        .flex_none()
                        .text_color(theme.text_faint)
                        .child("⋯"),
                )
                .child(div().min_w_0().text_color(theme.text_faint).child(text));
        }

        line.child(number(old))
            .child(number(new))
            .child(div().w(px(SIGN)).flex_none().text_color(mark).child(sign))
            .child(
                div()
                    .min_w_0()
                    .text_color(if change == Change::Context {
                        theme.text_muted
                    } else {
                        theme.text
                    })
                    .child(text),
            )
    }

    /// The header: the path, and how much moved. Counted from the rows rather
    /// than written down, so the numbers cannot disagree with the diff under
    /// them.
    fn header(theme: &Theme) -> gpui::Div {
        let count = |wanted: Change| ROWS.iter().filter(|(change, ..)| *change == wanted).count();
        let tally = |value: usize, sign: &'static str, color: gpui::Hsla| {
            div()
                .font_family(theme.font_mono.clone())
                .text_size(px(11.5))
                .text_color(color)
                .child(SharedString::from(format!("{sign}{value}")))
        };
        div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(8.0))
            .px(px(10.0))
            .py(px(7.0))
            .border_b_1()
            .border_color(theme.border)
            .child(
                icons::icon(icons::DOCUMENT)
                    .size(px(14.0))
                    .text_color(theme.text_muted),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .font_family(theme.font_mono.clone())
                    .text_size(px(12.0))
                    .text_color(theme.text)
                    .child(FILE),
            )
            .child(tally(count(Change::Added), "+", theme.success))
            .child(tally(count(Change::Removed), "−", theme.danger))
    }
}

impl Render for Diff {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::of(cx).clone();
        div()
            .size_full()
            .flex()
            .justify_center()
            .overflow_hidden()
            .child(
                div()
                    .w_full()
                    .max_w(px(760.0))
                    .px(px(24.0))
                    .py(px(32.0))
                    .child(
                        theme
                            .group_box()
                            .mt(px(0.0))
                            .child(Self::header(&theme))
                            .child(
                                div()
                                    .id("diff-rows")
                                    .overflow_x_scroll()
                                    .py(px(4.0))
                                    .flex()
                                    .flex_col()
                                    .children(ROWS.iter().map(|row| Self::row(&theme, *row))),
                            ),
                    ),
            )
    }
}
