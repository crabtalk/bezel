//! The block editor — the screen `editor` exists for.
//!
//! Nothing here is library code. It is two panes and a floating toolbar; the
//! calls into the library are `Editor`, `editor.source()` and three public
//! methods. Copy this file.
//!
//! Three things it is built to show.
//!
//! **The output is the document, live.** The right pane is
//! `editor.source()` — the document normalized and written back to markdown on
//! every keystroke. Nothing is transformed on the way to the screen, so what
//! you read there is exactly what a save would write.
//!
//! **The toolbar is a caller, not a component.** It floats at
//! `Editor::selection_bounds()` and each button is `toggle_mark` — the same
//! entry point cmd-B uses, so a button and a chord cannot disagree. Whether it
//! is lit comes from `Doc::covered_by`, which is the same question the toggle
//! asks itself before deciding to add or remove.
//!
//! **The rest needs no wiring at all.** The slash menu, the gutter handle,
//! drag-to-reorder, undo and the clipboard are the editor's own; this file
//! contains not one line for any of them.

use editor::{Anchor, CommentId, Editor, EditorEvent};
use gpui::{
    Context, ElementId, Entity, Focusable, Render, ScrollHandle, SharedString, Window, div,
    prelude::*, px,
};
use markdown::{Annotation, Mark};
use motion::{Fade, Painter};
use theme::{TextStyle, Theme, Typeset};

/// Opens on something worth selecting: a heading, a list, and a sentence with
/// marks already in it.
const SOURCE: &str = r#"# Notes

Select any of this and the toolbar appears. **Bold**, _italic_ and `code` are one keystroke or one button away, and they are the same call underneath.

- Type `/` on an empty line for the block menu
- Paste a URL on an empty line for a card, or into a sentence for a chip
- Hover a block and drag its handle to reorder it
- Everything on the right is what a save would write
- Click into the chart at the bottom to see the fence it really is

![A caption is the alt text, and a caret can sit in it](https://crabtalk.ai/og-home.png)

Drag a picture in from the desktop and it lands where the line says it will. `/image` makes an empty one that asks for a URL.

> A newline inside a block is a line break, here and in Notion both. This paragraph is one long line in the source, so it wraps to the pane instead.

A fence the app knows how to paint is a block of its own — put the caret in it to get the source back.

```chart
parse: 12
render: 47
paint: 31
```
"#;

/// The marks the toolbar offers, and the glyph each shows.
const MARKS: [(&str, Mark); 4] = [
    ("B", Mark::Bold),
    ("I", Mark::Italic),
    ("S", Mark::Strike),
    ("<>", Mark::Code),
];

/// The app's half of a comment. `editor` holds the range and maps it through
/// every edit; what it says, and who said it, lives here.
struct Thread {
    id: CommentId,
    body: SharedString,
}

pub struct EditorDemo {
    editor: Entity<Editor>,
    /// The threads, keyed to the editor's anchors by [`CommentId`].
    threads: Vec<Thread>,
    /// The one whose range paints as [`Annotation::Active`].
    open: Option<CommentId>,
    /// Ids are the app's to mint — the editor never looks inside one.
    next: u64,
    /// The document pane's scroll, shared with the editor so the caret can
    /// bring itself back into view.
    scroll: ScrollHandle,
    /// Focus lands in the document the first time this page paints. It is the
    /// one screen here whose whole point is that you type on it.
    focused: bool,
}

impl EditorDemo {
    pub fn new(cx: &mut Context<Self>) -> Self {
        // The pane scrolls, so the pane owns the handle — and the editor gets a
        // copy, which is how typing off the bottom follows the caret down.
        let scroll = ScrollHandle::new();
        let editor = cx.new({
            let scroll = scroll.clone();
            |cx| Editor::new(SOURCE, cx).with_scroll(scroll)
        });
        // Typing notifies the *editor*, and this screen reads two things off it
        // that have to keep up — the markdown pane and where the toolbar sits.
        // Without this the right half freezes on the opening text and the
        // toolbar never appears at all.
        cx.observe(&editor, |_, _, cx| cx.notify()).detach();
        // A press inside a comment's range opens its thread. Only the editor
        // sees that press, which is why this arrives as an event rather than as
        // a hit test this file would have to run itself.
        cx.subscribe(&editor, |this, _, event: &EditorEvent, cx| {
            if let EditorEvent::CommentActivated(id) = event {
                this.open = Some(*id);
                cx.notify();
            }
        })
        .detach();
        Self {
            editor,
            threads: Vec::new(),
            open: None,
            next: 0,
            scroll,
            focused: false,
        }
    }

    /// Comment on the selection: a thread here, an anchor there.
    fn comment(&mut self, cx: &mut Context<Self>) {
        let selection = self.editor.read(cx).selection();
        if selection.is_collapsed() {
            return;
        }
        let id = CommentId(self.next);
        self.next += 1;
        self.threads.push(Thread {
            id,
            body: format!("Note {}", self.threads.len() + 1).into(),
        });
        self.open = Some(id);
        self.editor.update(cx, |editor, cx| {
            let mut anchors = editor.anchors().to_vec();
            anchors.push(Anchor::new(id, selection));
            editor.set_anchors(anchors, cx);
        });
    }

    /// Repaint the anchors so the open thread is the lit one.
    fn light(&self, cx: &mut Context<Self>) {
        let open = self.open;
        self.editor.update(cx, |editor, cx| {
            let anchors = editor
                .anchors()
                .iter()
                .map(|anchor| Anchor {
                    state: if Some(anchor.id) == open {
                        Annotation::Active
                    } else {
                        Annotation::Open
                    },
                    ..anchor.clone()
                })
                .collect();
            editor.set_anchors(anchors, cx);
        });
    }

    /// The threads, each showing whether its words are still there.
    fn comments(&self, theme: &Theme, cx: &mut Context<Self>) -> Option<gpui::AnyElement> {
        if self.threads.is_empty() {
            return None;
        }
        let anchors = self.editor.read(cx).anchors().to_vec();
        let rows = self.threads.iter().map(|thread| {
            let (id, body) = (thread.id, thread.body.clone());
            let anchor = anchors.iter().find(|anchor| anchor.id == id);
            // The words it pointed at are gone. The thread is kept, because
            // whether that reads as outdated or resolved is this file's call.
            let detached = anchor.is_none_or(Anchor::detached);
            let open = self.open == Some(id);
            let range = anchor.map(|anchor| anchor.range);
            div()
                .id(ElementId::Name(format!("thread-{}", id.0).into()))
                .flex()
                .flex_row()
                .items_center()
                .gap(px(8.0))
                .px(px(10.0))
                .py(px(6.0))
                .rounded(px(6.0))
                .bg(if open {
                    theme.warning.opacity(0.14)
                } else {
                    theme.ink(0.02)
                })
                .text_style(TextStyle::Callout)
                .text_color(if detached {
                    theme.text_faint
                } else {
                    theme.text
                })
                .child(body)
                .child(
                    div()
                        .flex_none()
                        .ml_auto()
                        .text_style(TextStyle::Caption)
                        .text_color(theme.text_faint)
                        .child(if detached { "outdated" } else { "" }),
                )
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.open = Some(id);
                    // Clicking a thread selects the words it is about, which is
                    // what `Editor::select` is for.
                    if let Some(range) = range.filter(|_| !detached) {
                        this.editor
                            .update(cx, |editor, cx| editor.select(range, cx));
                    }
                    cx.notify();
                }))
        });
        Some(
            div()
                .flex()
                .flex_col()
                .gap(px(4.0))
                .children(rows)
                .into_any_element(),
        )
    }

    /// The toolbar, at the selection.
    ///
    /// Anchored in window coordinates through `popover::menu_at`, so this file
    /// never has to know where its own box starts.
    fn toolbar(&self, theme: &Theme, cx: &mut Context<Self>) -> Option<gpui::AnyElement> {
        let view = Painter::of(cx);
        let editor = self.editor.read(cx);
        let bounds = editor.selection_bounds()?;
        let (doc, selection) = (editor.doc().clone(), editor.selection());

        let buttons = MARKS.map(|(glyph, mark)| {
            let lit = doc.covered_by(selection, &mark);
            ui::popover::menu_row(theme, lit, Some(Fade::new(view, format!("bubble-{glyph}"))))
                .id(ElementId::Name(format!("bubble-{glyph}").into()))
                .px(px(7.0))
                .child(glyph)
                .on_click(cx.listener(move |this, _, _, cx| {
                    let mark = mark.clone();
                    this.editor
                        .update(cx, |editor, cx| editor.toggle_mark(mark, cx));
                }))
        });

        let comment = ui::popover::menu_row(theme, false, Some(Fade::new(view, "bubble-comment")))
            .id(ElementId::Name("bubble-comment".into()))
            .px(px(7.0))
            .child("Comment")
            .on_click(cx.listener(|this, _, _, cx| {
                this.comment(cx);
                this.light(cx);
            }));

        Some(ui::popover::menu_at(
            "bubble-toolbar",
            // *Below* the line, not above it. Above is the usual place and it
            // is wrong here: selecting the title puts the bubble over the pane's
            // own heading, and `menu_at` only snaps to the window, which is far
            // enough away to be no help.
            gpui::point(
                bounds.origin.x,
                bounds.origin.y + bounds.size.height + px(6.0),
            ),
            div()
                .flex()
                .flex_row()
                .gap(px(2.0))
                .children(buttons)
                .child(
                    div()
                        .w(px(1.0))
                        .h(px(16.0))
                        .mx(px(3.0))
                        .bg(theme.hairline(0.14)),
                )
                .child(comment)
                .into_any_element(),
            None,
        ))
    }
}

impl Render for EditorDemo {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::of(cx).clone();
        if !self.focused {
            self.focused = true;
            let handle = self.editor.read(cx).focus_handle(cx);
            handle.focus(window, cx);
        }
        let source = self.editor.read(cx).source();

        let pane = |label: &'static str| {
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .gap(px(8.0))
                .child(
                    div()
                        .text_style(TextStyle::Subheadline)
                        .text_color(theme.text_faint)
                        .child(label),
                )
        };

        let document = pane("EDITOR").child(
            div()
                .id("editor-demo-body")
                .flex_1()
                .min_h_0()
                .overflow_y_scroll()
                .track_scroll(&self.scroll)
                // Clicking the empty space below the last block should still
                // put a caret in the document, which is what filling the pane
                // with the editor buys.
                .child(self.editor.clone()),
        );

        let written = pane("MARKDOWN").child(
            div()
                .id("editor-demo-source")
                .flex_1()
                .min_h_0()
                .overflow_y_scroll()
                .p(px(12.0))
                .rounded(px(8.0))
                .bg(theme.ink(0.02))
                .font_family(theme.font_mono.clone())
                .text_style(TextStyle::Callout)
                .line_height(px(19.0))
                .text_color(theme.text_muted)
                .child(SharedString::from(source)),
        );

        let threads = self.comments(&theme, cx).map(|rows| {
            pane("COMMENTS").flex_none().max_h(px(160.0)).child(
                div()
                    .id("editor-demo-threads")
                    .overflow_y_scroll()
                    .child(rows),
            )
        });

        div()
            .size_full()
            .flex()
            .flex_col()
            .gap(px(12.0))
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .flex_row()
                    .gap(px(24.0))
                    .child(document)
                    .child(div().flex_none().w(px(1.0)).bg(theme.hairline(0.10)))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .gap(px(12.0))
                            .child(written)
                            .children(threads),
                    ),
            )
            .children(self.toolbar(&theme, cx))
            .on_mouse_down(
                gpui::MouseButton::Left,
                cx.listener(|this, _, window, cx| {
                    // A click anywhere on the page belongs to the editor: this
                    // is its screen, and a caret that needs hunting for is a
                    // worse demo than one that is always there.
                    let handle = this.editor.read(cx).focus_handle(cx);
                    handle.focus(window, cx);
                }),
            )
    }
}
