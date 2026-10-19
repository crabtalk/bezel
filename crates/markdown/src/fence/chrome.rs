//! The panel a fence is drawn in, painted or as code, and the copy button.

use gpui::{AnyElement, App, ElementId, SharedString, Window, div, prelude::*, px};
use theme::{TextStyle, Theme, Typeset};

/// What a fence holds its code in, inside its border.
pub(crate) const CODE_PADDING_X: f32 = 12.0;
pub(crate) const CODE_PADDING_Y: f32 = 10.0;

/// The box a fence is drawn in, painted or as code.
pub fn fence_panel(theme: &Theme) -> gpui::Div {
    div()
        .rounded(px(Theme::panel_radius()))
        .bg(theme.ink(0.035))
        .border_1()
        .border_color(theme.border)
        .overflow_hidden()
        .relative()
}

/// The debug selector on a [`fence_note`].
pub const NOTE_SELECTOR: &str = "fence-note";

/// A note across the bottom of a [`fence_panel`] under its code — why the
/// code was not painted — its lines kept as written.
pub fn fence_note(note: SharedString, theme: &Theme) -> AnyElement {
    div()
        .debug_selector(|| NOTE_SELECTOR.into())
        .flex()
        .flex_col()
        .px(px(CODE_PADDING_X))
        .py(px(CODE_PADDING_Y))
        .border_t_1()
        .border_color(theme.border)
        .text_color(theme.danger)
        .font_family(theme.font_mono.clone())
        .text_style(TextStyle::Subheadline)
        .children(
            note.lines()
                .map(|line| SharedString::from(line.to_string()))
                .collect::<Vec<_>>(),
        )
        .into_any_element()
}

/// The band across the top of a [`fence_panel`], where its label and copy
/// button sit.
pub fn fence_band(theme: &Theme) -> gpui::Div {
    div()
        .relative()
        .flex()
        .flex_row()
        .items_center()
        .px(px(CODE_PADDING_X))
        .py(px(5.0))
        .border_b_1()
        .border_color(theme.border)
        .bg(theme.ink(0.02))
        .text_style(TextStyle::Subheadline)
}

/// A copy button that owns its own feedback.
///
/// The state is the element's, not the caller's: a component library cannot ask
/// every host to thread a handler and a "which block is showing Copied" index
/// through its render tree just to put a button on a code block. It resets when
/// the pointer leaves, which needs no clock.
pub(crate) fn copy_button(
    code: &str,
    ix: usize,
    theme: &Theme,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    let copied = window.use_keyed_state(ElementId::named_usize("md-copied", ix), cx, |_, _| false);
    let showing = *copied.read(cx);
    let text: SharedString = code.to_string().into();

    div()
        .id(ElementId::named_usize("md-copy", ix))
        .absolute()
        .top(px(3.0))
        .right(px(5.0))
        .h(px(20.0))
        .px(px(6.0))
        .rounded(px(5.0))
        .flex()
        .items_center()
        .cursor_pointer()
        .text_style(TextStyle::Caption)
        .text_color(theme.text_muted)
        .hover(|el| el.bg(theme.element_hover))
        .child(if showing { "Copied" } else { "Copy" })
        .on_click({
            let copied = copied.clone();
            move |_, _, cx| {
                cx.write_to_clipboard(gpui::ClipboardItem::new_string(text.to_string()));
                copied.update(cx, |state, cx| {
                    *state = true;
                    cx.notify();
                });
            }
        })
        .on_hover(move |hovering, _, cx| {
            if !*hovering && *copied.read(cx) {
                copied.update(cx, |state, cx| {
                    *state = false;
                    cx.notify();
                });
            }
        })
        .into_any_element()
}
