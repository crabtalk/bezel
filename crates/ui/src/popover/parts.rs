//! Rows, headings, key hints and dialog primitives.

use super::*;

/// One menu row (the reference `menuItem`): `gap-2.5 rounded-lg px-2 py-1.5`,
/// active = `bg-white/10 text-foreground`. The caller adds the id/click
/// listener.
///
/// `active` is the row the cursor is on, and a menu has exactly one cursor.
/// `Some(fade)` lets the mouse light a row by itself, animated over
/// `transition-colors` (floating-styles.ts), for a menu holding no cursor of
/// its own; its key must be stable across frames (the id string is a good
/// choice). `None` is for a menu that owns an active index and moves it from
/// `on_mouse_move` — move, not hover: gpui settles hover at paint time, so a
/// list that re-filters or scrolls under a still mouse would drag the cursor
/// to wherever the pointer sits.
pub fn menu_row(theme: &Theme, active: bool, fade: Option<Fade>) -> gpui::Div {
    let row = div()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(10.0))
        .px(px(8.0))
        .py(px(MENU_ROW_PAD_Y))
        // Concentric with the card it sits in rather than a radius of its own:
        // 12 − 4 = 8, which is where the crate's most-repeated corner value
        // came from all along.
        .rounded(px(Theme::inset_radius(Theme::surface_radius(), MENU_PAD)))
        .text_style(TextStyle::Body)
        .cursor_pointer();
    match (active, fade) {
        (true, _) => row.bg(theme.card_selected_bg()).text_color(theme.text),
        (false, None) => row.text_color(theme.text.opacity(0.9)),
        (false, Some(fade)) => {
            let mut row = row
                .text_color(motion::hover_blend(
                    &fade,
                    theme.text.opacity(0.9),
                    theme.text,
                ))
                .bg(motion::hover_blend(
                    &fade,
                    theme::ink(0.0),
                    theme.element_hover,
                ));
            // Imperative form — the caller's `.id(...)` makes the element stateful
            // (hover listeners need element state, `.on_hover` needs `Stateful`).
            row.interactivity().on_hover(motion::hover_listener(fade));
            row
        }
    }
}

/// Small uppercase section heading inside a floating menu (the reference
/// `MenuHeading`): `px-2 pb-1 pt-1.5 uppercase tracking-[0.1em]
/// text-muted-foreground/60`. gpui has no letter-spacing at the pinned rev;
/// the tracking is approximated with hair spaces.
pub fn menu_heading(theme: &Theme, label: impl Into<SharedString>) -> gpui::Div {
    let label = label.into();
    div()
        .px(px(8.0))
        .pb(px(4.0))
        .pt(px(6.0))
        .text_style(TextStyle::Caption2)
        .text_color(theme.text_muted.opacity(0.6))
        .child(tracked_upper(&label))
}

/// Uppercase + hair-space tracking (see [`menu_heading`]).
pub fn tracked_upper(label: &str) -> String {
    let upper = label.to_uppercase();
    let mut out = String::with_capacity(upper.len() * 2);
    let mut first = true;
    for ch in upper.chars() {
        if !first {
            out.push('\u{200A}'); // hair space ≈ 0.1em tracking
        }
        out.push(ch);
        first = false;
    }
    out
}

/// Hairline divider between menu sections (the reference `MenuSeparator`:
/// `mx-1 my-1 h-px bg-white/[0.07]`).
pub fn divider() -> gpui::Div {
    // Full-bleed: negative margins cancel the card's p-1 inset so the hairline
    // runs border to border (user request).
    div().h(px(1.0)).mx(px(-4.0)).my(px(4.0)).bg(hairline(0.07))
}

/// The recessed band tone for a palette/picker header or footer strip — a
/// translucent black so the glass still reads through (the add-space palette
/// converged on this; measured subtler tones vanish against the dim scrim).
/// Free function (like [`ink`]/[`hairline`]/[`wash`]), mirroring
/// [`Theme::band`], for the several callers with no `Theme`/`cx` in scope
/// (some outside this crate's `ui` module tree — threading a `&Theme` param
/// would ripple past this task's file scope).
pub fn band() -> gpui::Hsla {
    theme::band()
}

/// One footer key-cap (22px, rounded-5, `white/[0.05]`) holding arbitrary
/// children — the base of [`key_hint`]/[`key_hint_pair`] and the search-bar
/// chips ("⌘K", "esc").
pub fn key_cap(_theme: &Theme) -> gpui::Div {
    div()
        .h(px(22.0))
        .px(px(5.0))
        .rounded(px(5.0))
        .flex()
        .flex_row()
        .items_center()
        .justify_center()
        .gap(px(4.0))
        .bg(ink(0.05))
}

/// The tiny verb after a key-cap.
pub(super) fn key_hint_label(theme: &Theme, label: &'static str) -> gpui::Div {
    div()
        .text_style(TextStyle::Caption)
        .text_color(theme.text_muted.opacity(0.45))
        .child(SharedString::from(label))
}

/// A footer legend: one icon key-cap + tiny verb (the add-space palette's
/// footer voice, shared by the pickers).
pub fn key_hint(theme: &Theme, icon: impl Into<Icon>, label: &'static str) -> gpui::Div {
    div()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(5.0))
        .child(
            key_cap(theme).child(
                crate::icons::icon(icon)
                    .size(px(12.5))
                    .text_color(theme.text_muted.opacity(0.7)),
            ),
        )
        .child(key_hint_label(theme, label))
}

/// A footer legend whose cap holds a WORD ("tab", "esc") instead of a glyph
/// — for keys with no icon in the set.
pub fn key_hint_text(
    theme: &Theme,
    cap: impl Into<SharedString>,
    label: &'static str,
) -> gpui::Div {
    div()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(5.0))
        .child(
            key_cap(theme)
                .text_style(TextStyle::Subheadline)
                .font_family(theme.font_mono.clone())
                .text_color(theme.text_muted.opacity(0.7))
                .child(cap.into()),
        )
        .child(key_hint_label(theme, label))
}

/// A footer legend whose cap holds TWO glyphs split by a hairline
/// ("[ ↑ | ↓ ] Navigate") sharing one verb.
pub fn key_hint_pair(
    theme: &Theme,
    first: impl Into<Icon>,
    second: impl Into<Icon>,
    label: &'static str,
) -> gpui::Div {
    div()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(5.0))
        .child(
            key_cap(theme)
                .child(
                    crate::icons::icon(first)
                        .size(px(12.5))
                        .text_color(theme.text_muted.opacity(0.7)),
                )
                .child(div().w(px(1.0)).h(px(11.0)).bg(hairline(0.10)))
                .child(
                    crate::icons::icon(second)
                        .size(px(12.5))
                        .text_color(theme.text_muted.opacity(0.7)),
                ),
        )
        .child(key_hint_label(theme, label))
}

/// A muted kbd hint chip inside menu rows (`⌘↵`-style accelerators).
pub fn kbd_hint(theme: &Theme, label: impl Into<SharedString>) -> gpui::Div {
    div()
        .flex_none()
        .px(px(5.0))
        .py(px(1.0))
        .rounded(px(5.0))
        .bg(ink(0.05))
        .text_style(TextStyle::Caption)
        .font_family(theme.font_mono.clone())
        .text_color(theme.text_muted.opacity(0.6))
        .child(label.into())
}

/// The query line at the top of a picker popover: a magnifier, the field, and
/// a hairline under it.
///
/// The field belongs in `with_frame(false)` — a box here would be a second
/// frame inside the card's. Full-bleed like [`divider`], and the glyph sits on
/// the row labels' own inset so the line reads as the head of the list rather
/// than a control dropped on top of it.
pub fn search_line(theme: &Theme, input: AnyElement) -> gpui::Div {
    stack::row()
        .mx(px(-MENU_PAD))
        .px(px(MENU_PAD + 8.0))
        .py(px(7.0))
        .mb(px(MENU_PAD))
        .border_b_1()
        .border_color(hairline(0.07))
        .text_style(TextStyle::Body)
        .child(
            icons::icon(icons::glyph::Search)
                .size(px(13.0))
                .text_color(theme.text_faint),
        )
        .child(div().flex_1().child(input))
}

/// A bordered trailing menu section (the reference picker action groups /
/// branch-picker worktree block: `mt-1 flex flex-col gap-0.5 border-t
/// border-white/[0.06] pt-1` — the hairline runs edge-to-edge of the card's
/// p-1 inset, unlike [`divider`]'s mx-1).
pub fn menu_section() -> gpui::Div {
    div()
        .mt(px(4.0))
        .pt(px(4.0))
        .border_t_1()
        .border_color(hairline(0.06))
        .flex()
        .flex_col()
        .gap(px(2.0))
}

// ---------------------------------------------------------------------------
// Dialog primitives (the reference dialog.tsx / sidebar dialogs.tsx)
// ---------------------------------------------------------------------------

/// The centered dialog card (`dialog-pop`): `w-[360px] rounded-2xl border
/// border-white/[0.1] bg-popover/95 p-5 shadow-2xl` — popover tone ≈ #101010.
pub fn dialog_card(theme: &Theme) -> gpui::Div {
    div()
        .w(px(360.0))
        .p(px(20.0))
        .rounded(px(DIALOG_RADIUS))
        .bg(theme.surface_dialog)
        .border_1()
        .border_color(hairline(0.10))
        .shadow_lg()
        .flex()
        .flex_col()
        .text_color(theme.text)
}

/// Dialog title.
pub fn dialog_title(theme: &Theme, title: impl Into<SharedString>) -> gpui::Div {
    div()
        .text_style(TextStyle::Headline)
        .text_color(theme.text)
        .child(title.into())
}

/// Dialog body copy: `leading-relaxed text-muted-foreground`.
pub fn dialog_body(theme: &Theme, copy: impl Into<SharedString>) -> gpui::Div {
    div()
        .text_style(TextStyle::Body)
        .line_height(px(19.0))
        .text_color(theme.text_muted)
        .child(copy.into())
}

/// Dialog text-field frame: `rounded-lg border border-white/[0.08]
/// bg-white/[0.04] px-3 py-2`.
pub fn dialog_field(input: AnyElement) -> gpui::Div {
    div()
        .w_full()
        .px(px(12.0))
        .py(px(8.0))
        .rounded(px(Theme::button_radius()))
        .border_1()
        .border_color(hairline(0.08))
        .bg(ink(0.04))
        .text_style(TextStyle::Body)
        .child(input)
}

/// Pulsing skeleton rows shown while a list loads (the reference:
/// `h-7 animate-pulse rounded-md bg-white/[0.04]`).
pub fn redacted_rows(
    _id: &'static str,
    _theme: &Theme,
    count: usize,
    painter: Painter,
    cx: &mut gpui::App,
) -> AnyElement {
    let wash = ink(0.04);
    let delta = motion::pulse_delta(&PULSE, painter, cx);
    div()
        .flex()
        .flex_col()
        .gap(px(6.0))
        .py(px(4.0))
        .children((0..count).map(move |i| {
            let phase = motion::staggered_phase(delta, i, 0.08);
            div()
                .h(px(28.0))
                .rounded(px(Theme::control_radius()))
                .bg(wash)
                .opacity(0.35 + 0.4 * motion::pulse_wave(phase))
        }))
        .into_any_element()
}

/// Inline error row + Retry affordance (the caller attaches the listener to the
/// returned id).
pub fn error_row(theme: &Theme, message: impl Into<SharedString>) -> gpui::Div {
    div()
        .flex()
        .flex_col()
        .gap(px(6.0))
        .p(px(Theme::SPACE))
        .text_style(TextStyle::Callout)
        .text_color(theme.danger)
        .child(message.into())
}
