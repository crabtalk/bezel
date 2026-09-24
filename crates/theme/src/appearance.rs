//! Light/dark switching: what the user asked for, what the OS reports, and the
//! plumbing that turns a change in either into a repaint.
//!
//! Three pieces, following the pattern zed uses (`crates/theme/src/theme.rs`
//! `SystemAppearance` + `reload_theme` + `cx.refresh_windows`):
//!
//! 1. [`AppearanceMode`] — the persisted user choice: follow the OS, or pin one.
//! 2. [`AppearanceState`] — a gpui global holding that choice alongside the last
//!    appearance the OS reported, so [`resolve`] can combine them.
//! 3. [`observe_window`] — subscribes to the platform's appearance notification
//!    (macOS `viewDidChangeEffectiveAppearance`) and re-applies.
//!
//! # Why `refresh_windows` and not `notify`
//!
//! Colors are read *imperatively* (`Theme::of(cx).text`) at paint time, not
//! through a reactive binding, so no view knows its colors went stale — a
//! `notify()` on some entity would repaint that entity and nothing else.
//! [`App::refresh_windows`] marks every window dirty *and* disables gpui's
//! per-view prepaint cache for the frame, which is the only thing that forces
//! already-laid-out elements to re-run their paint with the new palette.

use crate::{Appearance, Theme};
use gpui::{App, Decorations, Global, Subscription, Window, WindowBackgroundAppearance, WindowId};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// The user's appearance preference. Serde-serializable so callers can persist
/// it wherever their settings live; this crate never touches disk.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AppearanceMode {
    /// Follow the OS. The default — matches every other native app on the
    /// machine, including when the user has macOS set to switch at sunset.
    #[default]
    System,
    Light,
    Dark,
}

impl AppearanceMode {
    /// Menu/label text.
    pub fn label(self) -> &'static str {
        match self {
            Self::System => "System",
            Self::Light => "Light",
            Self::Dark => "Dark",
        }
    }

    pub const ALL: [Self; 3] = [Self::System, Self::Light, Self::Dark];
}

/// Global state behind the current theme: what the user chose, and what the OS
/// last said. Kept separate from [`Theme`] itself so that flipping the OS
/// appearance while the user has pinned Light still records the new system value
/// (and takes effect the moment they switch back to `System`).
pub struct AppearanceState {
    pub mode: AppearanceMode,
    pub system: Appearance,
}

impl Global for AppearanceState {}

/// Combine the user's choice with the OS state.
pub fn resolve(mode: AppearanceMode, system: Appearance) -> Appearance {
    match mode {
        AppearanceMode::System => system,
        AppearanceMode::Light => Appearance::Light,
        AppearanceMode::Dark => Appearance::Dark,
    }
}

/// Install the appearance globals and the matching theme. Call once at boot,
/// before any window opens, so the first frame is already the right palette
/// (installing later produces a visible dark-to-light flash).
pub fn init(mode: AppearanceMode, cx: &mut App) {
    let system = Appearance::from_window(cx.window_appearance());
    tracing::debug!(?mode, ?system, "appearance: initial");
    cx.set_global(AppearanceState { mode, system });
    sync_ns_appearance(mode);
    Theme::install(resolve(mode, system), cx);
}

/// The mode currently in effect (defaults to `System` before [`init`]).
pub fn mode(cx: &App) -> AppearanceMode {
    cx.try_global::<AppearanceState>()
        .map(|s| s.mode)
        .unwrap_or_default()
}

/// Change the user's preference and repaint if that changed the palette.
/// Persisting the choice is the caller's job.
pub fn set_mode(mode: AppearanceMode, cx: &mut App) {
    if !cx.has_global::<AppearanceState>() {
        return;
    }
    let state = cx.global_mut::<AppearanceState>();
    if state.mode == mode {
        return;
    }
    state.mode = mode;
    // Coming back to `System`, ask the OS what it actually is before resolving.
    // A pinned mode holds an `NSAppearance` over the app, and everything the
    // platform reports while one is up is that override read back — so
    // [`sync`] has been declining to record it and `system` is as stale as the
    // moment the mode was pinned. Clearing it here is what makes the read
    // honest; `apply` sets the same (cleared) override again a line later,
    // which keeps `sync_ns_appearance` the one place that owns it.
    if mode == AppearanceMode::System {
        sync_ns_appearance(mode);
        let system = Appearance::from_window(cx.window_appearance());
        cx.global_mut::<AppearanceState>().system = system;
    }
    apply(cx);
}

/// Subscribe a window to OS appearance changes. The returned [`Subscription`]
/// must outlive the window — callers typically `.detach()` it.
///
/// The notification is *per window*, but the appearance it reports is a system
/// setting, so any one window is enough to learn about the change; re-applying
/// is idempotent when several fire.
pub fn observe_window(window: &mut Window, cx: &mut App) -> Subscription {
    // Reconcile against the *window's* appearance before subscribing.
    //
    // [`init`] runs before any window exists and can only ask the platform
    // (`App::window_appearance`), which on macOS reads `NSApp.effectiveAppearance`
    // — and that is not reliably populated that early in launch. When it guesses
    // wrong the app paints the wrong palette until some unrelated event happens to
    // fire the appearance notification, which reads as "it booted dark and fixed
    // itself when I clicked something". The window knows for certain, so ask it.
    sync(Appearance::from_window(window.appearance()), cx);
    window.observe_window_appearance(|window, cx| {
        sync(Appearance::from_window(window.appearance()), cx);
    })
}

/// Whether what a window reports is the OS's own answer.
///
/// A pinned mode holds an `NSAppearance` over the app — see
/// [`sync_ns_appearance`] — and from then on every window reports that
/// override back. Only under `System` is there none in the way.
pub fn reports_the_os(mode: AppearanceMode) -> bool {
    matches!(mode, AppearanceMode::System)
}

/// Record the OS appearance and re-apply if it moved.
///
/// Only what [`reports_the_os`] will vouch for: recording an override read
/// back would overwrite what the OS said with what we asked for, and the first
/// switch to `System` would resolve to the mode just left. Nothing is lost by
/// skipping — a pinned mode ignores the OS anyway, and [`set_mode`] re-reads it
/// on the way back.
fn sync(system: Appearance, cx: &mut App) {
    if !cx.has_global::<AppearanceState>() {
        return;
    }
    let state = cx.global_mut::<AppearanceState>();
    if !reports_the_os(state.mode) || state.system == system {
        return;
    }
    tracing::debug!(?system, "appearance: system changed");
    state.system = system;
    apply(cx);
}

/// Re-resolve the palette and, if it moved, swap the theme and force a full
/// repaint. A no-op when the resolved appearance is unchanged — the OS fires the
/// notification for vibrancy and accent-color changes too, and repainting every
/// window for those would be a visible hitch for nothing.
pub fn apply(cx: &mut App) {
    let Some(state) = cx.try_global::<AppearanceState>() else {
        return;
    };
    sync_ns_appearance(state.mode);
    let wanted = resolve(state.mode, state.system);
    let changed = !cx
        .try_global::<Theme>()
        .is_some_and(|t| t.appearance == wanted);
    if changed {
        tracing::debug!(?wanted, "appearance: installing palette");
        Theme::install(wanted, cx);
        cx.refresh_windows();
    }
    // Unconditional, even when the palette did not move: this is the only thing
    // that keeps macOS vibrancy alive. gpui's macOS backend removes the
    // `NSVisualEffectView` from the window the moment the background appearance
    // is anything but `Blurred`, and nothing puts it back on its own — so a
    // single missed re-apply leaves the sidebar and tab strip permanently
    // opaque, which is exactly how the frost died. zed runs the same loop on
    // every settings change (`crates/zed/src/main.rs`).
    reapply_window_background(cx);
}

/// Tell AppKit which appearance the app's windows use, so the chrome *it*
/// draws — the traffic lights above all — matches the palette *we* paint.
/// gpui never sets `NSAppearance`, so before this a pinned in-app theme left
/// the window chrome following the OS setting: a light window rendered
/// dark-appearance inactive traffic lights when the system was dark (user
/// report). Pinned modes name the appearance explicitly; `System` clears the
/// override (`setAppearance: nil`) so AppKit follows the OS again — resolving
/// to a name there too would freeze the chrome across OS sunset switches
/// until our own notification round-trip repainted it.
#[cfg(target_os = "macos")]
fn sync_ns_appearance(mode: AppearanceMode) {
    use objc::{class, msg_send, runtime::Object, sel, sel_impl};
    // NSAppearanceName constants are NSStrings whose value equals the
    // constant's own name (AppKit documents them as stable identifiers), so
    // building them from literals avoids linking the extern statics.
    let name = match mode {
        AppearanceMode::System => None,
        AppearanceMode::Light => Some(c"NSAppearanceNameAqua"),
        AppearanceMode::Dark => Some(c"NSAppearanceNameDarkAqua"),
    };
    unsafe {
        let appearance: *mut Object = match name {
            None => std::ptr::null_mut(),
            Some(name) => {
                let name: *mut Object =
                    msg_send![class!(NSString), stringWithUTF8String: name.as_ptr()];
                msg_send![class!(NSAppearance), appearanceNamed: name]
            }
        };
        let app: *mut Object = msg_send![class!(NSApplication), sharedApplication];
        let _: () = msg_send![app, setAppearance: appearance];
    }
}

#[cfg(not(target_os = "macos"))]
fn sync_ns_appearance(_mode: AppearanceMode) {}

/// Windows that keep the background they opened with. See [`keep_background`].
#[derive(Default)]
struct KeepBackground(HashSet<WindowId>);

impl Global for KeepBackground {}

/// Leave this window's background where it is, whatever the palette says.
///
/// [`reapply_window_background`] reaches every open window, which is what keeps
/// vibrancy alive across an appearance switch. A window that is opaque *on
/// purpose* — a settings form the app behind it must not show through — says so
/// here rather than being frosted by the next switch.
pub fn keep_background(window: &Window, cx: &mut App) {
    let id = window.window_handle().window_id();
    cx.default_global::<KeepBackground>().0.insert(id);
}

/// Push the theme's window background appearance onto every open window, bar
/// the ones that asked to keep their own.
///
/// A window under `Decorations::Client` is pushed `Transparent` where the
/// theme answers `Opaque`: its corners and resize band (`ui::window::frame`)
/// must stay unpainted, and its content paints its own background.
pub fn reapply_window_background(cx: &mut App) {
    let Some(wanted) = cx
        .try_global::<Theme>()
        .map(|theme| theme.window_background_appearance())
    else {
        return;
    };
    let windows = cx.windows();
    let keep = if cx.has_global::<KeepBackground>() {
        let keep = cx.global_mut::<KeepBackground>();
        // The only place a closed window's id is dropped, which is enough:
        // `windows` is a handful, and the set is read here and nowhere else.
        keep.0
            .retain(|id| windows.iter().any(|window| window.window_id() == *id));
        keep.0.clone()
    } else {
        HashSet::new()
    };
    for window in windows {
        if keep.contains(&window.window_id()) {
            continue;
        }
        // A window cannot be updated from inside its own update — gpui takes
        // it out of its slot for the duration — and the OS appearance
        // notification arrives exactly that way, inside the observing window's
        // update. Pushing straight through would skip the one window that just
        // changed and leave it on the old background until something else set
        // one. Deferring runs it as the update unwinds, still before the frame.
        if window
            .update(cx, |_, window, _| {
                window.set_background_appearance(background_for(wanted, window));
            })
            .is_err()
        {
            cx.defer(move |cx| {
                window
                    .update(cx, |_, window, _| {
                        window.set_background_appearance(background_for(wanted, window));
                    })
                    .ok();
            });
        }
    }
}

/// `wanted`, bar an opaque background on a window that draws its own frame.
fn background_for(
    wanted: WindowBackgroundAppearance,
    window: &Window,
) -> WindowBackgroundAppearance {
    match (wanted, window.window_decorations()) {
        (WindowBackgroundAppearance::Opaque, Decorations::Client { .. }) => {
            WindowBackgroundAppearance::Transparent
        }
        _ => wanted,
    }
}
