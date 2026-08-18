//! The document pattern — a reader, and the screen `bezel-markdown` exists for.
//!
//! Nothing here is library code. The reader is an outline, a scroll area and a
//! toggle; the only call into the library is [`bezel_markdown::render`]. Copy
//! this file.
//!
//! Two things it is built to show.
//!
//! **The outline is a `filter`, not a walk.** A [`bezel_markdown::Doc`] is a
//! flat list of blocks carrying their own indent, so the table of contents is
//! one pass picking out headings — see [`Document::outline`]. On a nested
//! document tree the same list costs a recursive descent that has to
//! reconstruct depth on the way down. That is the whole argument for the flat
//! model, and it is the same reason the editor's Enter and Backspace are list
//! operations rather than restructures.
//!
//! **Source view is the round trip.** The Source segment does not show the
//! string this file holds — it shows `serialize(&doc)`, the document written
//! back out. It matches the original byte for byte, which is what makes an
//! edit/save cycle safe, and is the one property worth seeing rather than
//! reading about in a test.
//!
//! Like the other patterns it is an entity: a screen owns a screen's worth of
//! state, and its host holds one field.

use bezel_markdown::{BlockKind, Doc, Editor};
use bezel_theme::Theme;
use bezel_ui::widgets::Controls;
use gpui::{
    Context, ElementId, Entity, Focusable, Render, ScrollHandle, SharedString, Window, div,
    prelude::*, px,
};

/// The document on the page. Canonical markdown — `serialize(parse(SOURCE))`
/// returns it unchanged, which the gallery's tests assert, so the Source
/// segment can be compared against this by eye.
pub const SOURCE: &str = "\
# Markdown

Body text with **bold**, _italic_, ~~struck~~, `inline code`, and a \
[link](https://example.com).

## Lists

- A bullet
- Another one
    - Nested a level

1. First
2. Second

- [x] A finished task
- [ ] An unfinished one

## Quoting

> A quote, for the things worth setting apart.

## Code

```rust
fn main() {
    println!(\"hello\");
}
```

## Tables

| Column | Aligned right |
| --- | ---: |
| a | 1 |
| b | 22 |

---";

#[derive(Clone, Copy, PartialEq, Eq)]
enum View {
    Read,
    Edit,
    Source,
}

pub struct Document {
    editor: Entity<Editor>,
    view: View,
    scroll: ScrollHandle,
}

impl Document {
    /// The editor is an entity of its own, like every other screen's state: it
    /// owns a document and a caret, and this screen owns the editor.
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            editor: cx.new(|cx| Editor::new(SOURCE, cx)),
            view: View::Read,
            scroll: ScrollHandle::new(),
        }
    }
}

impl Document {
    /// The document the panes read from — the editor's once it has been
    /// touched, so Read and Source show what was typed rather than the
    /// constant this file opened with.
    fn current(&self, cx: &Context<Self>) -> Doc {
        self.editor.read(cx).doc().clone()
    }

    /// Every heading, with its level — the table of contents.
    ///
    /// One pass over the block list. There is no tree to descend and no depth
    /// to track, because a block already knows how deep it sits.
    fn outline(&self, doc: &Doc) -> Vec<(u8, SharedString)> {
        doc.blocks
            .iter()
            .filter_map(|block| match &block.kind {
                BlockKind::Heading { level, text } => {
                    Some((*level, SharedString::from(text.text.clone())))
                }
                _ => None,
            })
            .collect()
    }
}

impl Render for Document {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::of(cx).clone();
        let doc = self.current(cx);

        let outline = div()
            .flex_none()
            .w(px(180.0))
            .flex()
            .flex_col()
            .gap(px(6.0))
            .child(
                div()
                    .text_size(px(11.0))
                    .text_color(theme.text_faint)
                    .child("OUTLINE"),
            )
            .children(self.outline(&doc).into_iter().map(|(level, title)| {
                div()
                    .pl(px((level.saturating_sub(1) as f32) * 12.0))
                    .text_size(px(12.5))
                    .text_color(if level <= 1 {
                        theme.text
                    } else {
                        theme.text_muted
                    })
                    .child(title)
            }));

        let body = match self.view {
            View::Read => bezel_markdown::render(&doc, window, cx),
            View::Edit => self.editor.clone().into_any_element(),
            // The document written back out, not the constant above it.
            View::Source => div()
                .font_family(theme.font_mono.clone())
                .text_size(px(12.0))
                .line_height(px(19.0))
                .text_color(theme.text_muted)
                .child(SharedString::from(self.editor.read(cx).source()))
                .into_any_element(),
        };

        let segments = [
            ("Read", View::Read),
            ("Edit", View::Edit),
            ("Source", View::Source),
        ];
        let toggle = theme.toggle_group().children(segments.map(|(label, view)| {
            theme
                .toggle_group_item(label, self.view == view)
                .id(ElementId::Name(label.into()))
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.view = view;
                    if view == View::Edit {
                        let handle = this.editor.read(cx).focus_handle(cx);
                        handle.focus(window, cx);
                    }
                    cx.notify();
                }))
        }));

        div()
            .size_full()
            .flex()
            .flex_col()
            .gap(px(16.0))
            // No title here: the pane already heads the page with the rail's
            // own title and source path. A second one is just an echo.
            .child(div().flex().flex_row().justify_end().child(toggle))
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .flex_row()
                    .gap(px(28.0))
                    .child(outline)
                    .child(
                        div()
                            .id("document-body")
                            .flex_1()
                            .min_w_0()
                            .h_full()
                            .overflow_y_scroll()
                            .track_scroll(&self.scroll)
                            .child(div().max_w(px(680.0)).child(body)),
                    ),
            )
    }
}
