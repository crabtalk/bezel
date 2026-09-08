//! The selectable-text pattern — prose a reader can drag over and copy.
//!
//! Nothing here is library code. The only call into it is
//! [`markdown::selectable::render`], which paints a document and reports what
//! the pointer did; the selection, and what copying means, are this screen's.
//!
//! What it is built to show: **the selection is the caller's**. The library
//! knows a press, a drag and a release, and nothing else — so a screen with
//! several documents on it decides which one holds the selection, the way a
//! transcript's messages do. Here there is one document, so the state is one
//! `Option<Selection>`.
//!
//! Copy is [`markdown::selectable::copied`], which is plain text: the words a
//! reader dragged over, not the markup under them. The editor's own copy is a
//! different act — `slice` then `serialize` — because what it puts back is
//! blocks.

use gpui::{ClipboardItem, Context, Render, SharedString, Window, div, prelude::*, px};
use markdown::{
    BlockLayouts, Doc, Selection,
    selectable::{self, Pointer},
};
use theme::{TextStyle, Theme, Typeset};
use ui::widgets::{ButtonStyle, Buttons};

const SOURCE: &str = r#"## Selectable prose

Press anywhere in this text and drag. The selection is painted by the same
renderer the editor uses, and resolves against the same layouts — what is new
is only that nobody has to own an editor to get it.

- A drag that leaves the paragraph still selects to its end
- Crossing a block puts a newline where the block ended
- What copies out is the text, not the `**markup**` under it
"#;

pub struct Selectable {
    doc: Doc,
    /// Refilled by the renderer every frame, and what the next press resolves
    /// against.
    layouts: BlockLayouts,
    selection: Option<Selection>,
    /// A move only extends a selection a press started — otherwise the pointer
    /// would drag one just by crossing the text.
    dragging: bool,
    copied: Option<SharedString>,
}

impl Selectable {
    pub fn new(_: &mut Context<Self>) -> Self {
        Self {
            doc: markdown::parse(SOURCE),
            layouts: BlockLayouts::default(),
            selection: None,
            dragging: false,
            copied: None,
        }
    }

    /// Answer the pointer. A press starts a selection, a move drags its head,
    /// and a release leaves whatever it became.
    fn point(&mut self, pointer: Pointer, cx: &mut Context<Self>) {
        match pointer {
            Pointer::Down(cursor) => {
                self.selection = Some(Selection::at(cursor));
                self.dragging = true;
            }
            Pointer::Move(cursor) => {
                if let Some(selection) = self.selection {
                    self.selection = Some(selection.extend_to(cursor));
                }
            }
            Pointer::Up => self.dragging = false,
        }
        cx.notify();
    }

    /// What is selected, as it would be pasted — `None` for a press that
    /// collapsed with no drag behind it.
    fn text(&self) -> Option<String> {
        let text = selectable::copied(&self.doc, self.selection?);
        (!text.is_empty()).then_some(text)
    }
}

impl Render for Selectable {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::of(cx).clone();
        let text = self.text();
        let copy = theme
            .button("Copy", ButtonStyle::Prominent, None)
            .id("selectable-copy")
            .on_click(cx.listener(|view, _, _, cx| {
                if let Some(text) = view.text() {
                    cx.write_to_clipboard(ClipboardItem::new_string(text.clone()));
                    view.copied = Some(SharedString::from(text));
                    cx.notify();
                }
            }));
        div()
            .flex()
            .flex_col()
            .gap(px(16.0))
            .child(
                selectable::render(
                    "gallery-selectable",
                    &self.doc,
                    &self.layouts,
                    self.selection,
                    self.dragging,
                    window,
                    cx,
                    |view, pointer, cx| view.point(pointer, cx),
                )
                .into_any_element(),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(12.0))
                    .child(copy)
                    .child(
                        div()
                            .text_style(TextStyle::Callout)
                            .text_color(theme.text_muted)
                            .child(SharedString::from(match (&self.copied, &text) {
                                (Some(copied), _) => format!("copied: {copied:?}"),
                                (None, Some(text)) => format!("selected: {text:?}"),
                                (None, None) => "nothing selected".to_string(),
                            })),
                    ),
            )
    }
}
