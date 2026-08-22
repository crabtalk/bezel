//! Layout constants. Numbers drive layout, colors are paint: these live as
//! plain numbers and never depend on which color is painted.

use crate::theme::Theme;

impl Theme {
    // ---- numbers drive layout (px) ----
    /// Frost translucency over the blurred window background (macOS vibrancy).
    /// Opaque elsewhere: Linux/Windows get no compositor-blur guarantee, and a
    /// merely transparent window would show raw desktop through the sidebar.
    /// Darkness matched by eye to a reference Electron app's dark glass. That
    /// scrim is 0.76 over `hsl(0 0% 3%)`, but it sits on Electron's
    /// `under-window` vibrancy MATERIAL, which pre-darkens the blur; our bare
    /// backdrop blur has no material layer, so the scrim runs heavier to land
    /// on the same perceived tone (see [`Theme::glass`]).
    pub const GLASS_ALPHA: f32 = if cfg!(target_os = "macos") { 0.80 } else { 1.0 };
    /// Light-mode frost alpha — glass-forward, like dark mode.
    ///
    /// A light tint controls the blur less than a dark one: the desktop's
    /// colour bleeds through more readily, so light frost runs *heavier* than
    /// an equal-looking dark frost to keep the chrome on a known-enough
    /// background for its labels (macOS light sidebars do the same — their
    /// vibrancy material is mostly white). Floating cards compensate further:
    /// see [`Self::glass_overlay`], where light coverage steps up to keep menu
    /// text legible over an unknown backdrop.
    pub const GLASS_ALPHA_LIGHT: f32 = if cfg!(target_os = "macos") { 0.80 } else { 1.0 };
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
    /// Message bubble corner radius.
    pub const BUBBLE_RADIUS: f32 = 16.0;
    /// Floating-surface corner radius — popovers, menus, the command palette,
    /// group boxes.
    ///
    /// A glass surface paints this on its border **and** hands the same number
    /// to `bezel::ui::material`'s backdrop blur. The two must agree: a blur cut
    /// to a different radius frosts square corners outside a round border, and
    /// it shows only on glass and only at the corners. So the radius is named
    /// once and read at both ends, rather than written twice sixty lines apart
    /// — which is how three independent `12.0`s came to exist here.
    pub const SURFACE_RADIUS: f32 = 12.0;
    /// Panel / card corner radius.
    pub const PANEL_RADIUS: f32 = 10.0;
    /// Button, text field and select-trigger radius — the crate's most-used
    /// corner after the derived ones, and unnamed until the concentric pass
    /// separated the eight sites that *chose* 8.0 from the ones that only
    /// arrived at it as `12 − 4`.
    pub const BUTTON_RADIUS: f32 = 8.0;
    /// Small control radius (chips, tags, steppers) — a size down from
    /// [`Self::BUTTON_RADIUS`], for things that sit inside a control rather
    /// than being one.
    pub const CONTROL_RADIUS: f32 = 6.0;

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
