//! Layout constants. Numbers drive layout, colors are paint: these live as
//! plain numbers and never depend on which color is painted.

use std::sync::atomic::{AtomicU32, Ordering};

use crate::theme::Theme;

/// The branded base radius behind [`Theme::radius`], as raw `f32` bits.
static BASE: AtomicU32 = AtomicU32::new(Theme::BASE_RADIUS.to_bits());

/// The branded frost alpha behind [`Theme::glass`], as raw `f32` bits.
static FROST: AtomicU32 = AtomicU32::new(Theme::GLASS_ALPHA.to_bits());

/// What share of a glass shape's smaller side the lens profile spans.
/// Measured off SwiftUI's `.glassEffect(.clear)` (2026-08): on a 460x120
/// capsule the ruler behind it is displaced over the outer 27pt and is exactly
/// unperturbed below that — a bezel with a flat interior, not a lens across
/// the whole body.
static BEVEL: AtomicU32 = AtomicU32::new(0.225f32.to_bits());

/// The lens amplitude behind [`Theme::glass_magnify`], as raw `f32` bits.
static MAGNIFY: AtomicU32 = AtomicU32::new(0.34f32.to_bits());

/// Point the radius accessors at a base. Called by
/// [`Theme::install`](crate::theme::Theme::install).
pub(crate) fn set_base_radius(radius: f32) {
    BASE.store(radius.to_bits(), Ordering::Relaxed);
}

/// Point [`Theme::glass`] at a frost alpha. Called by
/// [`Theme::install`](crate::theme::Theme::install).
pub(crate) fn set_frost(alpha: f32) {
    FROST.store(alpha.to_bits(), Ordering::Relaxed);
}

/// Point [`Theme::glass_bevel`] at a share of the inradius.
pub fn set_glass_bevel(share: f32) {
    BEVEL.store(share.to_bits(), Ordering::Relaxed);
}

/// Point [`Theme::glass_magnify`] at an amplitude. Signed.
pub fn set_glass_magnify(amount: f32) {
    MAGNIFY.store(amount.to_bits(), Ordering::Relaxed);
}

pub(crate) fn magnify() -> f32 {
    f32::from_bits(MAGNIFY.load(Ordering::Relaxed))
}

pub(crate) fn bevel() -> f32 {
    f32::from_bits(BEVEL.load(Ordering::Relaxed))
}

pub(crate) fn frost() -> f32 {
    f32::from_bits(FROST.load(Ordering::Relaxed))
}

impl Theme {
    // ---- numbers drive layout (px) ----
    /// The frost alpha [`Brand::glass`](crate::Brand::glass) starts from.
    /// Matched by eye to a reference Electron app's dark glass: its scrim is
    /// 0.76 over `hsl(0 0% 3%)`, but sits on the `under-window` vibrancy
    /// MATERIAL, which pre-darkens the blur; a bare backdrop blur has no such
    /// layer, so ours runs heavier to land on the same perceived tone.
    ///
    /// Opaque off macOS: Linux and Windows get no compositor-blur guarantee,
    /// and a merely transparent window would show raw desktop through the
    /// sidebar. An app that knows its compositor sets the brand field anyway.
    pub const GLASS_ALPHA: f32 =
        if cfg!(any(target_os = "macos", target_family = "wasm")) { 0.80 } else { 1.0 };
    /// Main-panel header height (the reference `h-11`) — in-card headers (changes pane).
    pub const HEADER_HEIGHT: f32 = 44.0;
    /// The unified window titlebar (traffic lights + cluster + tabs). Content
    /// rides [`Self::TITLEBAR_TOP_PAD`] lower than center so the air above
    /// matches the perceived gap to the inset card below (border + card body).
    pub const TITLEBAR_HEIGHT: f32 = 38.0;
    /// Downward shift of titlebar content within the bar.
    pub const TITLEBAR_TOP_PAD: f32 = 2.0;
    /// Leading room the macOS traffic lights need where AppKit puts them —
    /// zed's `TRAFFIC_LIGHT_PADDING` on the macOS 26 SDK (71.0 before it), and
    /// the same 78 `../desktop` measured for its Tauri window. An app that
    /// *moves* the lights with `TitlebarOptions::traffic_light_position` owns
    /// this number too.
    pub const TRAFFIC_LIGHT_INSET: f32 = if cfg!(target_os = "macos") { 78.0 } else { 0.0 };
    /// Reserved status strip under the content outlet (the reference `h-6`) — the
    /// WorkingIndicator row; reserving it keeps the composer from shifting.
    pub const STATUS_STRIP_HEIGHT: f32 = 24.0;
    /// Height of the gradient that fades the transcript into the panel
    /// background at its bottom edge. The transcript's last row must pad
    /// itself past this band so settled content (message text, the
    /// hover-revealed timestamp) never sits inside the fade when scrolled
    /// to the bottom.
    pub const TRANSCRIPT_FADE_BAND: f32 = 24.0;
    /// Button, text field and select-trigger radius — the crate's most-used
    /// corner after the derived ones, and unnamed until the concentric pass
    /// separated the eight sites that *chose* 8.0 from the ones that only
    /// arrived at it as `12 − 4`.
    ///
    /// Every other corner is a ratio of this one, so
    /// [`Brand::radius`](crate::Brand::radius) moves the whole set together.
    pub const BASE_RADIUS: f32 = 8.0;

    /// Message bubble corner radius.
    pub fn bubble_radius() -> f32 {
        Self::radius(2.0)
    }
    /// Floating-surface corner radius — popovers, menus, the command palette,
    /// group boxes.
    ///
    /// A glass surface paints this on its border **and** hands the same number
    /// to `bezel::ui::material`'s backdrop blur. The two must agree: a blur cut
    /// to a different radius frosts square corners outside a round border, and
    /// it shows only on glass and only at the corners. So the radius is named
    /// once and read at both ends, rather than written twice sixty lines apart
    /// — which is how three independent `12.0`s came to exist here.
    pub fn surface_radius() -> f32 {
        Self::radius(1.5)
    }
    /// Panel / card corner radius.
    pub fn panel_radius() -> f32 {
        Self::radius(1.25)
    }
    /// Button, text field and select-trigger radius.
    pub fn button_radius() -> f32 {
        Self::radius(1.0)
    }
    /// Small control radius (chips, tags, steppers) — a size down from
    /// [`Self::button_radius`], for things that sit inside a control rather
    /// than being one.
    pub fn control_radius() -> f32 {
        Self::radius(0.75)
    }

    /// A corner as a multiple of the branded base radius.
    ///
    /// Read from a process-wide mirror rather than the theme global for the
    /// reason [`current_appearance`](crate::paint::current_appearance) is: the
    /// element builders that round a corner are free functions with no `cx` in
    /// scope, and a radius is one number for the whole app.
    fn radius(ratio: f32) -> f32 {
        f32::from_bits(BASE.load(Ordering::Relaxed)) * ratio
    }

    /// The concentric child of a surface: a row inset by `inset` inside a
    /// container of radius `outer` keeps its corners parallel to the
    /// container's, rather than looking pasted onto it.
    ///
    /// This is SwiftUI's `ContainerRelativeShape` rule done as arithmetic. gpui
    /// has no container shape to inherit at paint time, so the relationship is
    /// stated where the child is *defined* instead of resolved at runtime —
    /// which means a container that changes its padding carries its rows with
    /// it, and the derived value never becomes a constant of its own.
    pub const fn inset_radius(outer: f32, inset: f32) -> f32 {
        if outer > inset { outer - inset } else { 0.0 }
    }
    /// Base spacing steps.
    pub const SPACE_XS: f32 = 4.0;
    pub const SPACE_SM: f32 = 8.0;
    pub const SPACE_MD: f32 = 12.0;
    pub const SPACE_LG: f32 = 16.0;
}
