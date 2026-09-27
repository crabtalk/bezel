//! [`PendingKeys`] — the prefix of a key sequence gpui is holding, and what
//! can follow it.
//!
//! ```ignore
//! let pending = cx.new(|cx| PendingKeys::new(window, cx));
//! div().relative().child(content).child(
//!     div().absolute().bottom_2().right_2().child(pending.clone()),
//! )
//! ```

use std::rc::Rc;

use gpui::{
    Action, Context, IntoElement, KeybindingKeystroke, SharedString, Subscription, Window, div,
    prelude::*, px,
};

use theme::{TextStyle, Theme, Typeset};

use crate::{keys, popover, surface::Surfaced as _};

type Label = Rc<dyn Fn(&dyn Action) -> SharedString>;

/// Renders nothing while no sequence is pending in its window.
pub struct PendingKeys {
    label: Label,
    _pending: Subscription,
}

impl PendingKeys {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self {
            label: Rc::new(|action| label(action.name())),
            _pending: cx.observe_pending_input(window, |_, _, cx| cx.notify()),
        }
    }

    /// Names each continuation's action. The default is the action's name
    /// without its namespace, split into words: `pane::SplitRight` reads
    /// `Split right`.
    pub fn with_label(mut self, label: impl Fn(&dyn Action) -> SharedString + 'static) -> Self {
        self.label = Rc::new(label);
        self
    }
}

impl Render for PendingKeys {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let Some(held) = window.pending_input_keystrokes().map(<[_]>::to_vec) else {
            return div().into_any_element();
        };
        let theme = Theme::of(cx).clone();
        let prefix = keys::format(
            &held
                .iter()
                .cloned()
                .map(KeybindingKeystroke::from_keystroke)
                .collect::<Vec<_>>(),
        );
        let mut next: Vec<(SharedString, SharedString)> = Vec::new();
        for binding in window.possible_bindings_for_input(&held) {
            let rest = keys::format(&binding.keystrokes()[held.len()..]);
            // A shadowed binding on the same keys cannot fire.
            if !next.iter().any(|(keys, _)| *keys == rest) {
                next.push((rest, (self.label)(binding.action())));
            }
        }
        popover::popover_card(&theme)
            .flex()
            .flex_col()
            .gap(px(4.0))
            .text_style(TextStyle::Callout)
            .child(
                div()
                    .text_style(TextStyle::Subheadline)
                    .text_color(theme.text_faint)
                    .child(format!("{prefix} …")),
            )
            .children(next.into_iter().map(|(keys, label)| {
                div()
                    .flex()
                    .flex_row()
                    .gap(px(Theme::SPACE))
                    .child(div().min_w(px(48.0)).child(keys))
                    .child(div().text_color(theme.text_muted).child(label))
            }))
            .surface(&theme, theme.popover_surface)
            .into_any_element()
    }
}

/// `pane::SplitRight` → `Split right`.
fn label(name: &str) -> SharedString {
    let name = name.rsplit("::").next().unwrap_or(name);
    let mut out = String::with_capacity(name.len() + 4);
    for (index, char) in name.char_indices() {
        if char.is_uppercase() && index > 0 {
            out.push(' ');
            out.extend(char.to_lowercase());
        } else {
            out.push(char);
        }
    }
    out.into()
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_name_reads_as_words() {
        assert_eq!(super::label("pane::SplitRight").as_ref(), "Split right");
        assert_eq!(super::label("Close").as_ref(), "Close");
    }
}
