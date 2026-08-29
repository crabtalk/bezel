//! [`material`] — the material-glass float: wraps a popover/dialog card so its
//! ENTIRE subtree paints inside one scene layer (a single draw order).
//!
//! The single layer order is the point: with per-primitive bounds-tree
//! ordering, a hover repaint elsewhere can reassign the card's quads relative
//! to siblings — inside one layer the card's stacking is structural.
//!
//! The blur is painted first, structurally under the content: inside one layer
//! the order is blur, then shadow, tint, border, rows, text. It needs
//! `Window::paint_backdrop_blur` from our gpui fork (macOS Metal only);
//! elsewhere the primitive is ignored and the glass reads as the theme's
//! translucent tint over the OS window blur.
//!
//! [`Glass::glass_effect`] is the liquid surface: a bevel at the rim that
//! refracts the backdrop, and a fill of its own.

use gpui::{
    AbsoluteLength, AnyElement, App, Bounds, Corners, Element, GlobalElementId, InspectorElementId,
    IntoElement, LayoutId, Pixels, Styled, Window, fill, px,
};

use theme::Theme;

/// Backdrop-blur sigma for floating menu/dialog glass — the reference
/// `.glass-surface` runs `blur(44px)`, and the [`Theme::glass_overlay`] tint is
/// thin enough that a 16px blur leaves backdrop detail ghosting through rows.
pub const MENU_BLUR: f32 = 44.0;

/// Backdrop-blur sigma for a small floating panel — a meter, a HUD. A sigma is
/// only frost while the box is wide enough to show what it softened; at a
/// quarter of the box's width the backdrop resolves to one flat tone.
pub const PANEL_BLUR: f32 = 12.0;

/// Frost a card whose rounding the caller states — the shape the popover layers
/// need, where the card arrives already erased to an [`AnyElement`] and its
/// radius belongs to the surface kind. Backdrop-blurred on glass, pass-through
/// on opaque platforms.
pub fn material(corner_radius: f32, blur_radius: f32, child: impl IntoElement) -> Material {
    let radius = AbsoluteLength::from(px(corner_radius));
    Material {
        corners: Corners {
            top_left: radius,
            top_right: radius,
            bottom_right: radius,
            bottom_left: radius,
        },
        blur_radius,
        glass: false,
        tint: None,
        blur_override: None,
        child: child.into_any_element(),
    }
}

/// The backdrop surfaces, on any element carrying a corner radius.
pub trait Glass: Styled + IntoElement + Sized {
    /// Frost this card at the corner radius it already carries.
    ///
    /// The radius comes off the element's own style, so a caller chaining its
    /// own rounding — `card.rounded(px(4.0)).material(MENU_BLUR)` — gets a blur
    /// cut to the corners it just asked for.
    fn material(mut self, blur_radius: f32) -> Material {
        Material {
            corners: corners_of(&mut self),
            blur_radius,
            glass: false,
            tint: None,
            blur_override: None,
            child: self.into_any_element(),
        }
    }

    /// Paint this card as liquid glass — SwiftUI's `Glass.clear`: a refracting
    /// bevel at the rim, a lit edge, and [`Theme::glass_clear`]'s dimming.
    ///
    /// Blur, bevel and tint all come off [`Theme`], and the shape off the
    /// card's own rounding: the lens dies if any of the three is wrong.
    ///
    /// It clears the card's `bg`, since the lens paints the fill. Where the
    /// lens cannot run — every renderer but macOS Metal — it paints the frosted
    /// tint instead, so the surface is never invisible.
    fn glass_effect(mut self) -> Material {
        let corners = corners_of(&mut self);
        // The lens paints the fill, so the card's own is dropped here rather
        // than at the call site: painting both is what buries the lens, and a
        // caller who has to remember that is a caller who will forget.
        self.style().background = None;
        Material {
            corners,
            blur_radius: 0.0,
            glass: true,
            tint: None,
            blur_override: None,
            child: self.into_any_element(),
        }
    }
}

/// The rounding an element already carries, as the blur needs it.
fn corners_of(styled: &mut impl Styled) -> Corners<AbsoluteLength> {
    let square = AbsoluteLength::from(px(0.0));
    let radii = &styled.style().corner_radii;
    Corners {
        top_left: radii.top_left.unwrap_or(square),
        top_right: radii.top_right.unwrap_or(square),
        bottom_right: radii.bottom_right.unwrap_or(square),
        bottom_left: radii.bottom_left.unwrap_or(square),
    }
}

impl<E: Styled + IntoElement> Glass for E {}

pub struct Material {
    /// The glass's own tint, when the caller wants one. `None` is
    /// [`Theme::glass_clear`]'s neutral lift.
    tint: Option<gpui::Hsla>,
    /// Frost under the lens. Glass does not frost by default.
    blur_override: Option<f32>,
    /// Held unresolved: a rem-rounded card only becomes pixels against the
    /// window's rem size, which paint is the first place to have.
    corners: Corners<AbsoluteLength>,
    blur_radius: f32,
    glass: bool,
    child: AnyElement,
}

/// Whether this build has the backdrop-blur primitive behind it. Metal reads it
/// off the scene, and so does wgpu now that the fork implements it there —
/// which is why this tracks the gpui in use rather than the platform alone.
const LENSED: bool = cfg!(any(target_os = "macos", target_family = "wasm"));

/// Whether [`Glass::glass_effect`] will actually refract here, rather than
/// falling back to the flat backdrop tint. The primitive is macOS Metal's, and
/// an opaque appearance has nothing behind it to lens.
pub fn lensed(theme: &Theme) -> bool {
    LENSED && theme.is_glass()
}

impl Material {
    /// Tint the glass — SwiftUI's `Glass.tint(_:)`, for a control important
    /// enough to carry colour. Pass the colour at the coverage you want; it
    /// stands in for [`Theme::glass_clear`]'s neutral lift rather than adding
    /// to it, so a heavy alpha reads as paint and a light one as glass.
    ///
    /// A plain material's fill is still its caller's, and this does not
    /// reach it.
    pub fn tint(mut self, color: gpui::Hsla) -> Self {
        self.tint = Some(color);
        self
    }

    /// Frost the backdrop under the lens. Glass is clear by default — seeing
    /// through is the point — but a surface that has to carry dense content
    /// can buy legibility with a sigma of its own.
    pub fn blurred(mut self, sigma: f32) -> Self {
        self.blur_override = Some(sigma);
        self
    }

    /// The card's rounding in pixels, which only a rem size resolves.
    fn corners(&self, rem: Pixels) -> Corners<Pixels> {
        Corners {
            top_left: self.corners.top_left.to_pixels(rem),
            top_right: self.corners.top_right.to_pixels(rem),
            bottom_right: self.corners.bottom_right.to_pixels(rem),
            bottom_left: self.corners.bottom_left.to_pixels(rem),
        }
    }
}

impl Element for Material {
    type RequestLayoutState = ();
    type PrepaintState = ();

    fn id(&self) -> Option<gpui::ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, ()) {
        (self.child.request_layout(window, cx), ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) {
        self.child.prepaint(window, cx);
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        _prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        let theme = Theme::of(cx);
        // The backdrop-blur primitive is macOS Metal's alone. Glass moved the
        // card's fill into it, so anywhere the lens will not run the fill is
        // painted here — the overlay tint, so the card degrades to a surface
        // with the page showing through rather than to an opaque slab.
        if self.glass && !lensed(theme) {
            let corners = self.corners(window.rem_size());
            window.paint_quad(fill(bounds, theme.glass_overlay()).corner_radii(corners));
            // The lens is what would have carried the caller's colour.
            if let Some(tint) = self.tint {
                window.paint_quad(fill(bounds, tint).corner_radii(corners));
            }
        }
        if theme.is_glass() {
            // Only glass tints: a plain material paints what it always has,
            // and its caller still owns the fill.
            let (bevel, tint) = if self.glass {
                let extent = bounds.size.width.min(bounds.size.height);
                (
                    Theme::glass_bevel(f32::from(extent)),
                    self.tint.unwrap_or_else(|| theme.glass_clear()),
                )
            } else {
                (0.0, gpui::transparent_black())
            };
            let corners = self.corners(window.rem_size());
            window.paint_layer(bounds, |window| {
                window.paint_backdrop_blur(
                    bounds,
                    corners,
                    gpui::GlassEffect {
                        blur_radius: px(self.blur_override.unwrap_or(self.blur_radius)),
                        lens: px(bevel),
                        magnify: Theme::glass_magnify(),
                        dispersion: Theme::glass_dispersion(),
                        tint,
                    },
                );
                self.child.paint(window, cx);
            });
        } else {
            self.child.paint(window, cx);
        }
    }
}

impl IntoElement for Material {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

/// Paint `child` in its own scene layer, giving it a fresh draw order above
/// everything painted so far in the enclosing layer.
///
/// Needed for overlays INSIDE a material card: the card's single layer means
/// every primitive shares one draw order, and equal orders render grouped by
/// primitive kind (quads, then icons, then images) — so a close button's
/// circle painted "after" a thumbnail still shows up UNDER the image. A
/// nested layer restores the intended stacking.
pub fn layered(child: impl IntoElement) -> Layered {
    Layered {
        child: child.into_any_element(),
    }
}

pub struct Layered {
    child: AnyElement,
}

impl Element for Layered {
    type RequestLayoutState = ();
    type PrepaintState = ();

    fn id(&self) -> Option<gpui::ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, ()) {
        (self.child.request_layout(window, cx), ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) {
        self.child.prepaint(window, cx);
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        _prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        window.paint_layer(bounds, |window| self.child.paint(window, cx));
    }
}

impl IntoElement for Layered {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}
