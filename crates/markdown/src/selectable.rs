//! Text you can select with the pointer and copy out of.
//!
//! [`render`] handles pointer selection; [`surface`] adds keyboard focus and copy.
//! Selection state stays with the caller so lists can share one selection.

use crate::{BlockLayouts, Caret, Cursor, Doc, Editing, Selection, render_with};
use gpui::{
    AnyElement, Context, CursorStyle, DispatchPhase, ElementId, MouseButton, MouseDownEvent,
    MouseMoveEvent, Window, canvas, div, prelude::*,
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
/// text can know. It has to be `true` for at most one block at a time — a
/// block that is told it is dragging follows the pointer over the whole
/// window, so two of them would both extend on every move.
///
/// Releasing is answered twice over — on the text and off it — because a drag
/// that ends past the edge of a paragraph is the ordinary way to select to the
/// end of one. Moves are read the same way: `on_mouse_move` is delivered only
/// while the pointer is over this element's hitbox, so a drag off the text —
/// or under something painted over it — would otherwise stop extending where
/// it crossed the edge. [`BlockLayouts::hit`] resolves a point outside the
/// text to the nearest line, which is what makes the off-hitbox move
/// answerable at all.
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
        // Registered in the paint phase, from a canvas that occupies nothing:
        // a window listener is the only one that hears a move the hitbox does
        // not cover, and `Window::on_mouse_event` may only be called there.
        .children(dragging.then(|| {
            let view = cx.entity();
            canvas(
                |_, _, _| (),
                move |_, _, window, _| {
                    window.on_mouse_event(move |event: &MouseMoveEvent, phase, _, cx| {
                        if phase != DispatchPhase::Bubble
                            || event.pressed_button != Some(MouseButton::Left)
                        {
                            return;
                        }
                        if let Some(cursor) = at_move.hit(event.position) {
                            view.update(cx, |view, cx| {
                                moved(view, Pointer::Move(cursor), cx);
                            });
                        }
                    });
                },
            )
            .absolute()
            .size_0()
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
                caret: selection.map(Caret::new),
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

gpui::actions!(selectable, [Copy]);

struct Bindings;
impl gpui::Global for Bindings {}

/// Wrap selectable content with focus-on-click and platform copy shortcuts.
/// Rebuild when the document or selection changes. Keep `focus` stable per surface.
pub fn surface(
    focus: &gpui::FocusHandle,
    doc: &Doc,
    selection: Option<Selection>,
    cx: &mut gpui::App,
) -> gpui::Div {
    if !cx.has_global::<Bindings>() {
        let chord = if cfg!(target_os = "macos") {
            "cmd-c"
        } else {
            "ctrl-c"
        };
        cx.bind_keys([gpui::KeyBinding::new(chord, Copy, Some("SelectableText"))]);
        cx.set_global(Bindings);
    }
    let text = selection
        .map(|selection| copied(doc, selection))
        .unwrap_or_default();
    let focus = focus.clone();
    div()
        .key_context("SelectableText")
        .track_focus(&focus)
        .on_mouse_down(MouseButton::Left, move |_, window, cx| {
            window.focus(&focus, cx)
        })
        .on_action(move |_: &Copy, _, cx| {
            if !text.is_empty() {
                cx.write_to_clipboard(gpui::ClipboardItem::new_string(text.clone()));
            }
        })
}
