//! [`Item`] — a row in a dropped menu — and [`card`], the panel that paints a
//! list of them.
//!
//! Every menu in the system is those two over the caller's own open state: the
//! bar's dropped panel ([`crate::menubar`]), the `···` on a row, the picker
//! under a chip. The state stays with the caller because only it knows what
//! opening means — a [`crate::popover::Popup`] for one, a field for another.

use crate::{icons, popover};
use gpui::{Context, SharedString, Window, div, prelude::*, px};
use motion::{Fade, Painter};
use std::rc::Rc;
use theme::{TextStyle, Theme, Typeset};

/// The leading glyph and the trailing check, at the size the rows are set in.
const GLYPH: f32 = 13.0;

/// A row in a menu.
///
/// Deliberately not a struct with an `is_separator` flag: a separator has no
/// label, no accelerator and nothing to enable, and every one of those fields
/// would have to be answered anyway.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Item {
    Action {
        label: SharedString,
        /// The leading glyph's asset path — [`crate::icons`]' consts, or a
        /// path the app resolved at runtime. A menu where no row has one keeps
        /// no room for it.
        icon: Option<SharedString>,
        /// The accelerator to *print* — the binding itself is the app's, and
        /// bezel never dispatches it. A menu that showed a keystroke it did not
        /// own would be documenting a lie.
        keystroke: Option<SharedString>,
        /// The choice the menu is currently on, marked with a trailing check.
        checked: bool,
        enabled: bool,
    },
    Separator,
}

impl Item {
    pub fn action(label: impl Into<SharedString>) -> Self {
        Item::Action {
            label: label.into(),
            icon: None,
            keystroke: None,
            checked: false,
            enabled: true,
        }
    }

    /// No-ops on a separator, which has nothing to hang a glyph on.
    pub fn with_icon(self, icon: impl Into<SharedString>) -> Self {
        match self {
            Item::Action {
                label,
                keystroke,
                checked,
                enabled,
                ..
            } => Item::Action {
                label,
                icon: Some(icon.into()),
                keystroke,
                checked,
                enabled,
            },
            Item::Separator => Item::Separator,
        }
    }

    /// No-ops on a separator, which has nothing to hang a keystroke on.
    pub fn with_keystroke(self, keystroke: impl Into<SharedString>) -> Self {
        match self {
            Item::Action {
                label,
                icon,
                checked,
                enabled,
                ..
            } => Item::Action {
                label,
                icon,
                keystroke: Some(keystroke.into()),
                checked,
                enabled,
            },
            Item::Separator => Item::Separator,
        }
    }

    /// Takes the flag, because what a menu is on is decided per render.
    pub fn checked(self, checked: bool) -> Self {
        match self {
            Item::Action {
                label,
                icon,
                keystroke,
                enabled,
                ..
            } => Item::Action {
                label,
                icon,
                keystroke,
                checked,
                enabled,
            },
            Item::Separator => Item::Separator,
        }
    }

    pub fn disabled(self) -> Self {
        match self {
            Item::Action {
                label,
                icon,
                keystroke,
                checked,
                ..
            } => Item::Action {
                label,
                icon,
                keystroke,
                checked,
                enabled: false,
            },
            Item::Separator => Item::Separator,
        }
    }

    /// Whether the keyboard and the pointer can land here at all.
    pub fn selectable(&self) -> bool {
        matches!(self, Item::Action { enabled: true, .. })
    }

    fn has_icon(&self) -> bool {
        matches!(self, Item::Action { icon: Some(_), .. })
    }
}

/// The next row the keyboard can land on, `delta` deciding the direction:
/// separators and disabled rows are stepped straight over, and both ends wrap.
/// `from` of `None` enters the menu at the edge the direction comes from.
///
/// [`popover::menu_step`] cannot do this — it counts rows and knows nothing
/// about which of them can be landed on. `None` back means *nothing* in the menu
/// is selectable, which is the one shape that would otherwise spin forever.
pub fn next_selectable(items: &[Item], from: Option<usize>, delta: isize) -> Option<usize> {
    let count = items.len();
    if count == 0 {
        return None;
    }
    let step = if delta >= 0 { 1 } else { -1 };
    let wrap = |at: usize| (at as isize + step).rem_euclid(count as isize) as usize;
    // Entering, the first candidate is the edge itself; moving, it is the row
    // after the one you are on.
    let mut at = match from {
        None if step > 0 => 0,
        None => count - 1,
        Some(at) => wrap(at.min(count - 1)),
    };
    for _ in 0..count {
        if items[at].selectable() {
            return Some(at);
        }
        at = wrap(at);
    }
    None
}

/// The panel a menu drops: every [`Item`] as a row, in a
/// [`popover::popover_card`]. `id` prefixes the rows' element ids, so two menus
/// open at once keep their hover state apart.
///
/// `highlighted` is where the keyboard is. The pointer's own row is the hover
/// fade, so a menu nobody can arrow through passes `None`.
///
/// Dismissal is the caller's `.on_mouse_down_out` on the returned card: what
/// closing means is the caller's state, not the panel's.
pub fn card<V: 'static>(
    theme: &Theme,
    id: impl Into<SharedString>,
    items: &[Item],
    highlighted: Option<usize>,
    cx: &mut Context<V>,
    choose: impl Fn(&mut V, usize, &mut Window, &mut Context<V>) + 'static,
) -> gpui::Div {
    let id = id.into();
    let painter = Painter::of(cx);
    // A menu where nothing carries a glyph keeps no room for one — a bar's
    // menus would otherwise open with an empty column down their left.
    let gutter = items.iter().any(Item::has_icon);
    let choose = Rc::new(choose);
    popover::popover_card(theme)
        .min_w(px(180.0))
        .children(items.iter().enumerate().map(|(index, item)| {
            let Item::Action {
                label,
                icon,
                keystroke,
                checked,
                enabled,
            } = item
            else {
                return popover::divider().into_any_element();
            };
            let row = if *enabled {
                let choose = choose.clone();
                popover::menu_row(
                    theme,
                    highlighted == Some(index),
                    Some(Fade::new(painter, format!("{id}-{index}"))),
                )
                .id(SharedString::from(format!("{id}-{index}")))
                .on_click(cx.listener(move |view, _, window, cx| choose(view, index, window, cx)))
            } else {
                disabled_row(theme).id(SharedString::from(format!("{id}-{index}")))
            };
            row.when(gutter, |row| {
                row.child(glyph_slot(theme, icon.clone(), *enabled))
            })
            .child(div().flex_1().min_w_0().child(label.clone()))
            .when(*checked, |row| {
                row.child(
                    icons::icon(icons::CHECK)
                        .size(px(GLYPH))
                        .text_color(theme.text),
                )
            })
            .when_some(keystroke.clone(), |row, keystroke| {
                row.child(popover::kbd_hint(theme, &keystroke))
            })
            .into_any_element()
        }))
}

/// The leading column: the row's glyph, or the room one would have taken, so a
/// menu of mixed rows keeps its labels on one edge.
fn glyph_slot(theme: &Theme, icon: Option<SharedString>, enabled: bool) -> gpui::Div {
    div()
        .flex_none()
        .size(px(GLYPH))
        .flex()
        .items_center()
        .justify_center()
        .children(icon.map(|path| {
            gpui::svg()
                .path(path)
                .size(px(GLYPH))
                .text_color(if enabled {
                    theme.text_faint
                } else {
                    theme.text_faint.opacity(0.5)
                })
        }))
}

/// A row that cannot be chosen: [`popover::menu_row`]'s metrics without its
/// hover fade or its click, because a disabled row that lit under the pointer
/// would be inviting a press that does nothing.
fn disabled_row(theme: &Theme) -> gpui::Div {
    div()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(10.0))
        .px(px(8.0))
        .py(px(6.0))
        // The disabled twin of `popover::menu_row`, in the same card — so it
        // takes its corners from the same rule rather than a matching literal.
        .rounded(px(Theme::inset_radius(
            Theme::surface_radius(),
            popover::MENU_PAD,
        )))
        .text_style(TextStyle::Body)
        .text_color(theme.text_faint)
}
