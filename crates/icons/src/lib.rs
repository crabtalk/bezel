//! icons — the bezel icon set. Reached as `bezel::icons`.
//!
//! Icons are grouped into categories, one module and one cargo feature each:
//!
//! ```ignore
//! use ui::icons::{self, system};
//!
//! Application::new().with_assets(icons::Assets);
//!
//! icons::icon(system::MAGNIFER).size(px(16.0)).text_color(theme.text_muted)
//! ```
//!
//! [`icon`] returns a gpui `Svg`, so it colors with `text_color` and sizes like
//! any element. The paths are `&'static str`, which is what makes the set
//! browsable: [`CATEGORIES`] is every category and its icons, and that is what
//! the gallery's icon page renders.
//!
//! # Where the glyphs come from
//!
//! Nothing here is drawn by hand or checked in, and nothing is downloaded. The
//! declaration below *is* the library: one line per icon, naming ours and the
//! [Lucide](https://lucide.dev) glyph behind it. Cargo pulls those glyphs in as
//! ordinary Rust source — `icondata_lu` stores each one as a `static` holding
//! the path data — so there is no asset to fetch, no file to write, and no
//! build script. `document` assembles the SVG gpui asks for, on demand.
//!
//! That means the set works offline, under `cargo vendor` and on docs.rs,
//! pinned byte-for-byte by `Cargo.lock`. We name 84 of that crate's 1599
//! statics and the linker drops the rest, so the icons an app never paints cost
//! it nothing. Lucide is ISC-licensed and asks for no attribution in a binary.
//!
//! Adding one is a line in the declaration. The `LuFoo` path is checked by the
//! compiler, so a glyph that does not exist upstream is an error naming the bad
//! identifier rather than a blank square at runtime.
//!
//! Our names are ours: `MAGNIFER` stays `MAGNIFER` though Lucide calls it
//! `LuSearch`, because the set is named for the *shape* — the app's own word
//! for what the shape does stays in the app.
//!
//! # Paying for what you paint
//!
//! Every category is on by default. `default-features = false` narrows the set,
//! and the icons you leave out are never compiled. Cargo unions features across
//! a graph, so a category a dependency turns on is one you cannot turn off —
//! `ui` needs four of them, and those four are the floor for anything
//! depending on it.

use std::{borrow::Cow, fmt::Write as _};

use gpui::{Styled as _, Svg, svg};
use icondata_core::IconData;

/// Declares the set: categories of named icons, each sourced from a Lucide
/// glyph, expanding to the modules, their constants and [`Assets`].
///
/// A category names itself twice — `arrows = "arrows"` — because a module needs
/// an identifier and `#[cfg(feature = ..)]` needs a literal, and `stringify!`
/// cannot cross into an attribute. An icon names itself twice for the same
/// class of reason in reverse: `macro_rules!` cannot mint `ARROW_LEFT` from
/// `"arrow-left"`, since building an identifier out of a string is the one
/// thing only a procedural macro can do. That is not worth a second published
/// crate, so the pair is written out and `constants_match_their_paths` in the
/// tests holds the two halves together.
macro_rules! icon_set {
    // The only word an entry may end with. These two rules turn its presence
    // into the flag `document` takes.
    (@solid) => { false };
    (@solid solid) => { true };

    ($(
        $(#[$doc:meta])*
        $category:ident = $feature:literal {
            $(($name:ident, $file:literal, $glyph:ident $(, $solid:ident)?)),* $(,)?
        }
    )*) => {
        $(
            #[cfg(feature = $feature)]
            $(#[$doc])*
            pub mod $category {
                $(
                    #[doc = concat!("Lucide `", stringify!($glyph), "`, served at `",
                        "icons/", stringify!($category), "/", $file, ".svg`.")]
                    pub const $name: &str =
                        concat!("icons/", stringify!($category), "/", $file, ".svg");
                )*

                /// Every icon in this category: `(constant name, asset path)`.
                pub const ALL: &[(&str, &str)] = &[$((stringify!($name), $name)),*];
            }
        )*

        /// Every enabled category: `(category name, that category's `ALL`)`.
        pub const CATEGORIES: &[(&str, &[(&str, &str)])] = &[$(
            #[cfg(feature = $feature)]
            ($feature, $category::ALL),
        )*];

        /// Every enabled icon's asset path.
        pub const PATHS: &[&str] = &[$($(
            #[cfg(feature = $feature)]
            $category::$name,
        )*)*];

        // The match below is the only caller of `document`. Naming its type
        // here states that contract in one place and keeps the function from
        // reading as dead code in a build that enables no category at all.
        const _: fn(&IconData, bool) -> Cow<'static, [u8]> = document;

        /// Serves the set to gpui's SVG renderer.
        pub struct Assets;

        impl ::gpui::AssetSource for Assets {
            fn load(&self, path: &str) -> ::gpui::Result<Option<Cow<'static, [u8]>>> {
                Ok(match path {
                    $($(
                        #[cfg(feature = $feature)]
                        $category::$name => {
                            Some(document(::icondata_lu::$glyph, icon_set!(@solid $($solid)?)))
                        }
                    )*)*
                    _ => None,
                })
            }

            fn list(&self, path: &str) -> ::gpui::Result<Vec<::gpui::SharedString>> {
                Ok(PATHS
                    .iter()
                    .filter(|candidate| candidate.starts_with(path))
                    .map(|candidate| ::gpui::SharedString::from(*candidate))
                    .collect())
            }
        }
    };
}

icon_set! {
    /// Direction. The four cardinals move a selection; the `ALT_ARROW_*`
    /// chevrons disclose — a menu, a tree row, a step. `RETURN` is the key rather
    /// than a direction, and it lives here because that is where a reader looks.
    arrows = "arrows" {
        (ARROW_LEFT, "arrow-left", LuArrowLeft),
        (ARROW_RIGHT, "arrow-right", LuArrowRight),
        (ARROW_UP, "arrow-up", LuArrowUp),
        (ARROW_DOWN, "arrow-down", LuArrowDown),
        (ALT_ARROW_LEFT, "alt-arrow-left", LuChevronLeft),
        (ALT_ARROW_RIGHT, "alt-arrow-right", LuChevronRight),
        (ALT_ARROW_DOWN, "alt-arrow-down", LuChevronDown),
        (RETURN, "return", LuCornerDownLeft),
        (EXPAND_ARROWS, "expand-arrows", LuMaximize2),
        (FOLD_VERTICAL, "fold-vertical", LuChevronsDownUp),
        (SORT_VERTICAL, "sort-vertical", LuArrowUpDown),
    }
    /// Transport, and the three volume glyphs a level control swaps between as
    /// you slide. They are one family on purpose: a speaker cone that changed
    /// shape mid-slide would read as a bug.
    media = "media" {
        (PLAY, "play", LuPlay),
        (PLAY_BOLD, "play-bold", LuPlay, solid),
        (PAUSE, "pause", LuPause),
        (PAUSE_BOLD, "pause-bold", LuPause, solid),
        (STOP, "stop", LuSquare),
        (SKIP_NEXT, "skip-next", LuSkipForward),
        (SKIP_PREVIOUS, "skip-previous", LuSkipBack),
        (SHUFFLE, "shuffle", LuShuffle),
        (REPEAT, "repeat", LuRepeat),
        (REPEAT_ONE, "repeat-one", LuRepeat1),
        (PLAYLIST, "playlist", LuListMusic),
        (MICROPHONE, "microphone", LuMic),
        (VOLUME_MUTE, "volume-mute", LuVolumeX),
        (VOLUME_LOW, "volume-low", LuVolume1),
        (VOLUME_LOUD, "volume-loud", LuVolume2),
        (HEART, "heart", LuHeart),
        (HEART_BOLD, "heart-bold", LuHeart, solid),
    }
    /// Documents and folders, and the four things you do to them — archive,
    /// download, copy, discard.
    files = "files" {
        (DOCUMENT, "document", LuFileText),
        (DOCUMENT_ADD, "document-add", LuFilePlus),
        (FOLDER, "folder", LuFolder),
        (FOLDER_WITH_FILES, "folder-with-files", LuFolderOpen),
        (ARCHIVE_MINIMALISTIC, "archive-minimalistic", LuArchive),
        (ARCHIVE_UP_MINIMALISTIC, "archive-up-minimalistic", LuArchiveRestore),
        (DOWNLOAD, "download", LuDownload),
        (COPY, "copy", LuCopy),
        (TRASH_BIN_MINIMALISTIC, "trash-bin-minimalistic", LuTrash2),
        (BOOK, "book", LuBook),
        (TAG, "tag", LuTag),
        (PAPERCLIP, "paperclip", LuPaperclip),
        (LINK, "link", LuLink),
    }
    /// Hardware and the network it reaches: the screens an app runs on, the
    /// machine under it, the connection out.
    devices = "devices" {
        (MONITOR, "monitor", LuMonitor),
        (LAPTOP, "laptop", LuLaptop),
        (SMARTPHONE, "smartphone", LuSmartphone),
        (KEYBOARD, "keyboard", LuKeyboard),
        (COMMAND, "command", LuCommand),
        (CPU, "cpu", LuCpu),
        (TERMINAL, "terminal", LuTerminal),
        (WIFI_OFF, "wifi-off", LuWifiOff),
        (GLOBAL, "global", LuGlobe),
        (CLOUD, "cloud", LuCloud),
    }
    /// Writing and structuring text — the pens, the lists, and the branch a
    /// change lands on.
    editing = "editing" {
        (PEN, "pen", LuPen),
        (PEN_NEW_SQUARE, "pen-new-square", LuSquarePen),
        (TEXT, "text", LuType),
        (HASHTAG, "hashtag", LuHash),
        (LIST, "list", LuList),
        (CHECKLIST, "checklist", LuListChecks),
        (GIT_BRANCH, "git-branch", LuGitBranch),
    }
    /// What a surface says back: confirmation, information, warning,
    /// dismissal.
    status = "status" {
        (CHECK, "check", LuCheck),
        (INFO_CIRCLE, "info-circle", LuInfo),
        (DANGER_TRIANGLE, "danger-triangle", LuTriangleAlert),
        (CLOSE_CIRCLE, "close-circle", LuCircleX),
        (BELL, "bell", LuBell),
        (STAR, "star", LuStar),
        (STAR_BOLD, "star-bold", LuStar, solid),
    }
    /// Chrome and controls — the search field, the sidebar toggles, the
    /// settings, and the two ends of the appearance switch.
    system = "system" {
        (CLOSE, "close", LuX),
        (PLUS, "plus", LuPlus),
        (ADD_CIRCLE, "add-circle", LuCirclePlus),
        (MENU_DOTS, "menu-dots", LuEllipsis),
        (MAGNIFER, "magnifer", LuSearch),
        (REFRESH, "refresh", LuRefreshCw),
        (RESTART, "restart", LuRotateCcw),
        (SETTINGS_MINIMALISTIC, "settings-minimalistic", LuSettings),
        (TUNING, "tuning", LuSlidersHorizontal),
        (WIDGET, "widget", LuLayoutGrid),
        (SIDEBAR_MINIMALISTIC, "sidebar-minimalistic", LuPanelRight),
        (SIDEBAR_MINIMALISTIC_LEFT, "sidebar-minimalistic-left", LuPanelLeft),
        (LOGOUT_2, "logout-2", LuLogOut),
        (KEY_MINIMALISTIC, "key-minimalistic", LuKey),
        (CALENDAR, "calendar", LuCalendar),
        (CHAT_ROUND_LINE, "chat-round-line", LuMessageCircle),
        (COMPASS, "compass", LuCompass),
        (SUN, "sun", LuSun),
        (MOON, "moon", LuMoon),
    }
}

/// Paints one Lucide glyph into a complete SVG document.
///
/// `icondata` stores the glyph's *body* plus the attributes that belong on the
/// root, which is not something resvg can render on its own — in particular
/// `fill="none"`, without which every stroked path fills solid. gpui paints an
/// SVG as an alpha mask tinted by `text_color`, so `currentColor` never needs
/// to resolve to anything.
///
/// Called on a sprite-atlas miss, once per icon per size, immediately before
/// gpui parses and rasterizes the result — which is why assembling the document
/// here rather than storing 84 of them costs nothing worth measuring.
fn document(glyph: &IconData, solid: bool) -> Cow<'static, [u8]> {
    let mut svg = String::from("<svg xmlns=\"http://www.w3.org/2000/svg\"");

    attribute(&mut svg, "viewBox", glyph.view_box);
    // A solid twin is the same path painted in: filling *and* stroking keeps
    // its outer edge exactly where the outline twin's is, so a control that
    // swaps between them — play/pause, favorited or not — does not jump.
    if solid {
        attribute(&mut svg, "fill", Some("currentColor"));
        attribute(&mut svg, "stroke", Some("currentColor"));
    } else {
        attribute(&mut svg, "fill", glyph.fill);
        attribute(&mut svg, "stroke", glyph.stroke);
    }
    attribute(&mut svg, "stroke-width", glyph.stroke_width);
    attribute(&mut svg, "stroke-linecap", glyph.stroke_linecap);
    attribute(&mut svg, "stroke-linejoin", glyph.stroke_linejoin);

    svg.push('>');
    svg.push_str(glyph.data);
    svg.push_str("</svg>");
    Cow::Owned(svg.into_bytes())
}

fn attribute(svg: &mut String, name: &str, value: Option<&str>) {
    if let Some(value) = value {
        write!(svg, " {name}=\"{value}\"").expect("writing to a String cannot fail");
    }
}

/// An icon element for a path from the set. Size and color are the caller's
/// (`.size(..)`, `.text_color(..)`), matching the `[&_svg]:size-4` idiom.
pub fn icon(path: &'static str) -> Svg {
    svg().path(path).flex_none()
}
