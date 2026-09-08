//! Text you can select with the pointer and copy out of.
//!
//! The two hard halves are already here: [`render_with`] paints a [`Selection`]
//! it is handed, and fills a [`BlockLayouts`] whose [`BlockLayouts::hit`] turns
//! a point back into a [`Cursor`]. What was missing is the *gesture* — press,
//! drag, release — which belonged to whoever owns the selection, and until now
//! only an editor ever did.
//!
//! This is that gesture and nothing else. Which item holds the selection, and
//! what copying means, stay the caller's.

use crate::{BlockLayouts, Cursor, Doc, Editing, Selection, render_with};
use gpui::{
    AnyElement, Context, CursorStyle, ElementId, MouseButton, MouseDownEvent, MouseMoveEvent,
    Window, div, prelude::*,
};
use std::rc::Rc;

/// What the pointer did over the text.
pub enum Pointer {
    /// Pressed here — the start of a selection.
    Down(Cursor),
    /// Moved here with the button still down.
    Move(Cursor),
    /// Let go. Whatever the selection had become is what it is.
    Up,
}

/// Render `doc` with `selection` painted in it, reporting what the pointer does.
///
/// `dragging` is the caller's: a move only extends a selection that a press
/// started, and which item that press landed in is not something one block of
/// text can know.
///
/// Releasing is answered twice over — on the text and off it — because a drag
/// that ends past the edge of a paragraph is the ordinary way to select to the
/// end of one.
#[expect(
    clippy::too_many_arguments,
    reason = "a document, its selection, and a gesture"
)]
pub fn render<V: 'static>(
    id: impl Into<ElementId>,
    doc: &Doc,
    layouts: &BlockLayouts,
    selection: Option<Selection>,
    dragging: bool,
    window: &mut Window,
    cx: &mut Context<V>,
    on_pointer: impl Fn(&mut V, Pointer, &mut Context<V>) + 'static,
) -> AnyElement {
    let on_pointer = Rc::new(on_pointer);
    let (down, moved, up, off) = (
        on_pointer.clone(),
        on_pointer.clone(),
        on_pointer.clone(),
        on_pointer,
    );
    let (at_down, at_move) = (layouts.clone(), layouts.clone());
    div()
        .id(id)
        .cursor(CursorStyle::IBeam)
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(move |view, event: &MouseDownEvent, _, cx| {
                if let Some(cursor) = at_down.hit(event.position) {
                    down(view, Pointer::Down(cursor), cx);
                }
            }),
        )
        .on_mouse_move(cx.listener(move |view, event: &MouseMoveEvent, _, cx| {
            if dragging && let Some(cursor) = at_move.hit(event.position) {
                moved(view, Pointer::Move(cursor), cx);
            }
        }))
        .on_mouse_up(
            MouseButton::Left,
            cx.listener(move |view, _, _, cx| up(view, Pointer::Up, cx)),
        )
        .on_mouse_up_out(
            MouseButton::Left,
            cx.listener(move |view, _, _, cx| off(view, Pointer::Up, cx)),
        )
        .child(render_with(
            doc,
            Editing {
                selection,
                // Read-only text has no caret. Without this a collapsed
                // selection — every press that starts one — would blink an
                // insertion point in text nobody can type into.
                caret_on: false,
                layouts: Some(layouts),
                ..Editing::default()
            },
            window,
            cx,
        ))
        .into_any_element()
}

/// The text a selection covers, as it would be pasted.
///
/// [`Doc::spans`] answers in parts — a paragraph, a cell, a line of a fence —
/// and a newline between them is what puts a multi-block selection back
/// together.
pub fn copied(doc: &Doc, selection: Selection) -> String {
    doc.spans(selection)
        .into_iter()
        .filter_map(|(at, range)| {
            let text = &doc.blocks.get(at.block)?.text_at(at.part)?.text;
            text.get(range).map(str::to_owned)
        })
        .collect::<Vec<_>>()
        .join("\n")
}
