//! The markdown bezel reads and writes, spelling beside result.
//!
//! Nothing here is library code. Every row on the right is
//! [`markdown::render`] over the source on its left, so the page cannot drift
//! from the dialect it documents — a spelling that stopped working stops
//! painting. Copy this file.

use gpui::{Context, Render, ScrollHandle, SharedString, Window, div, prelude::*, px};
use markdown::Doc;
use motion::Painter;
use theme::{TextStyle, Theme, Typeset};
use ui::scroll::{self, Axes, TransientState};

use crate::{hint, stack};

/// A group of spellings, and what each one is called.
struct Group {
    title: &'static str,
    note: &'static str,
    rows: &'static [(&'static str, &'static str)],
}

/// The whole vocabulary, in the order a reader meets it. Each entry is a label
/// and the markdown that spells it.
const GROUPS: &[Group] = &[
    Group {
        title: "BLOCKS",
        note: "A block is one line's worth of prefix. Type the prefix and the block turns as you \
               finish it, which is the same vocabulary a paste is read with.",
        rows: &[
            ("Heading", "# Heading one\n\n### Heading three"),
            (
                "Paragraph",
                "Plain text, and a newline inside it\nis a line break.",
            ),
            ("Bullet", "- Bullet\n- Another"),
            ("Ordered", "1. First\n2. Second"),
            ("Task", "- [ ] To do\n- [x] Done"),
            ("Quote", "> Quoted."),
            ("Rule", "---"),
        ],
    },
    Group {
        title: "INLINE",
        note: "Typing the closing delimiter is what makes the mark, so `**bold**` becomes bold on \
               the last asterisk. Marks nest and the order survives a round trip.",
        rows: &[
            ("Bold", "**bold**"),
            ("Italic", "_italic_ and *italic*"),
            ("Strikethrough", "~~struck~~"),
            ("Code", "`inline code`"),
            ("Link", "A [link](https://bezel.gallery) in a sentence."),
        ],
    },
    Group {
        title: "FENCES",
        note: "A fence tag names the grammar `syntax` highlights it with — and a tag an app paints \
               itself is a block of its own, with the fence as its source.",
        rows: &[
            ("Plain", "```\nno language\n```"),
            ("Tagged", "```rs\nfn main() {}\n```"),
            ("Painted", "```chart\nparse: 12\nrender: 47\n```"),
        ],
    },
    Group {
        title: "LINKS AND PICTURES",
        note: "A link alone on a line is a card rather than a sentence with a link in it. The \
               title slot picks which of the three shapes, since markdown has no other spelling \
               for it.",
        rows: &[
            ("Bookmark", "<https://bezel.gallery>"),
            (
                "Chip",
                "[https://bezel.gallery](https://bezel.gallery \"chip\")",
            ),
            (
                "Picture",
                "![A caption is the alt text](https://crabtalk.ai/og-home.png)",
            ),
            (
                "Sized",
                "![Dragged narrower, in whole pixels|240](https://crabtalk.ai/og-home.png)",
            ),
        ],
    },
    Group {
        title: "TABLES",
        note: "GFM's table, alignment row included. Every cell is one line, and a caret sits in \
               each of them.",
        rows: &[(
            "Table",
            "| Left | Middle | Right |\n| :--- | :----: | ----: |\n| a | b | c |",
        )],
    },
    Group {
        title: "NESTING",
        note: "Four spaces per level, and the indent counts list nesting only. A document is a \
               flat list of blocks carrying a depth, so a quote inside a list flattens to the \
               two blocks it reads as — which is Notion's model, and the reason an edit is a \
               list operation rather than a restructure.",
        rows: &[(
            "Indent",
            "- Top level\n    - One level in\n        1. And an ordered list under that",
        )],
    },
];

pub struct Dialect {
    /// Parsed once: a markdown parse per row per frame would put the whole
    /// vocabulary in the scroll path.
    docs: Vec<Vec<Doc>>,
    scroll: ScrollHandle,
    bar: TransientState,
}

impl Dialect {
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            docs: GROUPS
                .iter()
                .map(|group| {
                    group
                        .rows
                        .iter()
                        .map(|(_, source)| markdown::parse(source))
                        .collect()
                })
                .collect(),
            scroll: ScrollHandle::new(),
            bar: TransientState::new(Painter::of(cx)),
        }
    }
}

impl Render for Dialect {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::of(cx).clone();

        let groups = GROUPS.iter().zip(&self.docs).map(|(group, docs)| {
            let rows = group.rows.iter().zip(docs).map(|((label, source), doc)| {
                div()
                    .flex()
                    .flex_row()
                    .items_start()
                    .gap(px(24.0))
                    .py(px(10.0))
                    .border_t_1()
                    .border_color(theme.hairline(0.08))
                    .child(
                        div()
                            .flex_none()
                            .w(px(300.0))
                            .flex()
                            .flex_col()
                            .gap(px(4.0))
                            .child(
                                div()
                                    .text_style(TextStyle::Caption2)
                                    .text_color(theme.text_faint)
                                    .child(SharedString::from(*label)),
                            )
                            .child(
                                div()
                                    .font_family(theme.font_mono.clone())
                                    .text_style(TextStyle::Callout)
                                    .line_height(px(19.0))
                                    .text_color(theme.text_muted)
                                    .child(SharedString::from(*source)),
                            ),
                    )
                    .child(div().flex_1().min_w_0().child(markdown::render(
                        doc,
                        markdown::Caption::Shown,
                        window,
                        cx,
                    )))
            });

            stack()
                .gap(px(8.0))
                .child(
                    div()
                        .text_style(TextStyle::Subheadline)
                        .text_color(theme.text_faint)
                        .child(SharedString::from(group.title)),
                )
                .child(hint(&theme, group.note).max_w(px(680.0)))
                .children(rows)
        });

        div()
            .relative()
            .size_full()
            .child(
                scroll::pane("dialect-page", Axes::Vertical)
                    .size_full()
                    .track_scroll(&self.scroll)
                    .child(
                        stack()
                            .child(hint(
                                &theme,
                                "The markdown `markdown::parse` reads and `markdown::serialize` \
                                 writes. Every row on the right is the source on its left, \
                                 rendered — so what a save would write is what you see spelled \
                                 out here.",
                            ))
                            .children(groups),
                    ),
            )
            .child(scroll::transient(
                "dialect-bar",
                &self.scroll,
                &self.bar,
                cx.reduce_motion(),
            ))
    }
}
