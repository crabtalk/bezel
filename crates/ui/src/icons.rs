//! Embedded icon assets + the gpui [`AssetSource`] that serves them.
//!
//! Most glyphs come from the **Solar Icons** set (Linear weight) by 480 Design,
//! licensed under CC BY 4.0 (https://creativecommons.org/licenses/by/4.0/);
//! attribution: "Solar Icons by 480 Design". A few (`terminal`, `plus`,
//! `close`, `stop`, `git-branch`, `star`) are hand-drawn in the same style.
//!
//! Icons render via [`icon`]: `icon(icons::PAPERCLIP).size(px(16.)).text_color(…)`.

use std::borrow::Cow;

use gpui::{AssetSource, Result, SharedString, Styled as _, Svg, svg};

macro_rules! icon_assets {
    ($(($const_name:ident, $path:literal)),+ $(,)?) => {
        $(pub const $const_name: &str = concat!("icons/", $path, ".svg");)+

        /// Every icon in the set: `(constant name, asset path)`. The paths are
        /// `&'static str` so they can be handed straight to [`icon`] — which is
        /// what makes the set browsable at all.
        pub const ALL: &[(&str, &str)] = &[
            $((stringify!($const_name), concat!("icons/", $path, ".svg"))),+
        ];

        /// Serves the embedded icons to gpui's SVG renderer.
        pub struct Assets;

        impl AssetSource for Assets {
            fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
                Ok(match path {
                    $(concat!("icons/", $path, ".svg") => Some(Cow::Borrowed(
                        include_bytes!(concat!("../assets/icons/", $path, ".svg")).as_slice(),
                    )),)+
                    _ => None,
                })
            }

            fn list(&self, path: &str) -> Result<Vec<SharedString>> {
                let all = [$(concat!("icons/", $path, ".svg")),+];
                Ok(all
                    .iter()
                    .filter(|p| p.starts_with(path))
                    .map(|p| SharedString::from(*p))
                    .collect())
            }
        }
    };
}

icon_assets![
    // Solar Icons (Linear), CC BY 4.0 — 480 Design.
    (MONITOR, "monitor"),
    (LAPTOP, "laptop"),
    (PEN_NEW_SQUARE, "pen-new-square"),
    (SORT_VERTICAL, "sort-vertical"),
    (LIST, "list"),
    (FOLDER_WITH_FILES, "folder-with-files"),
    (FOLDER, "folder"),
    // Hand-drawn git-branch glyph in the Solar Linear style (like the
    // terminal/plus/return ports) — the set has no branch icon.
    (GIT_BRANCH, "git-branch"),
    // Compact history-ref glyphs, drawn in the same linear style.
    (CLOUD, "cloud"),
    (TAG, "tag"),
    (SIDEBAR_MINIMALISTIC, "sidebar-minimalistic"),
    // Mirrored variant (the reference window-controls.tsx `-scale-x-100`): the LEFT
    // sidebar toggle shows the panel line on the left; gpui divs have no
    // scale transform at the pinned rev, so the flip is baked into the asset.
    (SIDEBAR_MINIMALISTIC_LEFT, "sidebar-minimalistic-left"),
    (KEY_MINIMALISTIC, "key-minimalistic"),
    (KEYBOARD, "keyboard"),
    (ARROW_LEFT, "arrow-left"),
    (ARROW_RIGHT, "arrow-right"),
    (ARROW_UP, "arrow-up"),
    // arrow-up mirrored (like the sidebar flip) — the Solar Linear set here
    // has no plain arrow-down.
    (ARROW_DOWN, "arrow-down"),
    // Hand-drawn return/enter arrow in the Solar Linear style (like the
    // terminal/plus/close ports) — the set has no return glyph.
    (RETURN, "return"),
    (ALT_ARROW_DOWN, "alt-arrow-down"),
    // Hand-drawn expand/maximize arrows in the Solar Linear style (like the
    // terminal/plus/return ports) — the set has no expand glyph.
    (EXPAND_ARROWS, "expand-arrows"),
    // Hand-drawn fold-all chevrons, drawn as a family with EXPAND_ARROWS
    // (same stroke, caps, 90° joints) — Solar has no unfold-less either.
    (FOLD_VERTICAL, "fold-vertical"),
    (ALT_ARROW_LEFT, "alt-arrow-left"),
    (ALT_ARROW_RIGHT, "alt-arrow-right"),
    (SMARTPHONE, "smartphone"),
    (ARCHIVE_UP_MINIMALISTIC, "archive-up-minimalistic"),
    (REFRESH, "refresh"),
    (RESTART, "restart"),
    (ADD_CIRCLE, "add-circle"),
    (TUNING, "tuning"),
    (PAPERCLIP, "paperclip"),
    (PEN, "pen"),
    (ARCHIVE_MINIMALISTIC, "archive-minimalistic"),
    (TRASH_BIN_MINIMALISTIC, "trash-bin-minimalistic"),
    (SETTINGS_MINIMALISTIC, "settings-minimalistic"),
    (LOGOUT_2, "logout-2"),
    (MAGNIFER, "magnifer"),
    (COMMAND, "command"),
    (DOCUMENT, "document"),
    (DOCUMENT_ADD, "document-add"),
    (GLOBAL, "global"),
    (CHECKLIST, "checklist"),
    (WIDGET, "widget"),
    (WIFI_OFF, "wifi-off"),
    (CLOSE_CIRCLE, "close-circle"),
    // Hand-drawn info glyph in the Solar Linear style (like the terminal/
    // plus/return ports) — the embedded set has no info-circle.
    (INFO_CIRCLE, "info-circle"),
    (DANGER_TRIANGLE, "danger-triangle"),
    (CHAT_ROUND_LINE, "chat-round-line"),
    // Hand-drawn bell + speaker in the Solar Linear style (like the terminal/
    // plus/return ports) — the embedded set has neither.
    (BELL, "bell"),
    (VOLUME_LOUD, "volume-loud"),
    // Hand-drawn reference glyphs (terminal-panel.tsx / composer-actions.tsx /
    // menu-check.tsx / logo.tsx).
    (TERMINAL, "terminal"),
    (PLUS, "plus"),
    (CLOSE, "close"),
    (STOP, "stop"),
    (CHECK, "check"),
    (COPY, "copy"),
    // Hand-drawn star pair in the Solar Linear style (like the terminal/
    // plus/return ports) — outline for the favorite affordance, bold for the
    // favorited state and the picker's favorites rail tab.
    (STAR, "star"),
    (STAR_BOLD, "star-bold"),
];

/// An icon element for an embedded asset path. Size and colour are set by the
/// caller (`.size(..)`, `.text_color(..)`), matching the web app's
/// `[&_svg]:size-4` idiom.
pub fn icon(path: &'static str) -> Svg {
    svg().path(path).flex_none()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_registered_icon_loads_and_parses() {
        let assets = Assets;
        for path in assets.list("icons/").unwrap() {
            let bytes = assets
                .load(&path)
                .unwrap()
                .unwrap_or_else(|| panic!("missing asset {path}"));
            let text = std::str::from_utf8(&bytes).expect("icon svg is utf-8");
            assert!(text.contains("<svg"), "{path} is not an svg");
            assert!(text.contains("viewBox"), "{path} lacks a viewBox");
        }
    }

    #[test]
    fn unknown_paths_are_none() {
        assert!(Assets.load("icons/nope.svg").unwrap().is_none());
    }

    #[test]
    fn list_filters_by_prefix() {
        assert!(!Assets.list("icons/").unwrap().is_empty());
        assert!(Assets.list("fonts/").unwrap().is_empty());
    }
}
