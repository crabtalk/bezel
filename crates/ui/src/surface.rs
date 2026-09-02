//! [`crate::surface`] — the backdrop surface a floating card sits on: wraps a
//! popover/dialog card so its ENTIRE subtree paints inside one scene layer (a
//! single draw order).
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
//! Material and glass are different things — a material has thickness, a glass
//! has a variant — and they meet only at the numbers they resolve to, which is
//! why one element paints both and [`theme::SurfaceStyle`] names which.

use gpui::{
    AbsoluteLength, AnyElement, App, Bounds, Corners, Element, GlobalElementId, InspectorElementId,
    IntoElement, LayoutId, Pixels, Styled, Window, fill, px,
};

use theme::{SurfaceSpec, SurfaceStyle, Theme};

/// Paint an already-composed card as a surface, at the rounding the caller
/// states — the shape the popover layers need, where the card arrives erased to
/// an [`AnyElement`] and cannot be chained onto. The look comes off whichever
/// theme paints it; a caller wanting its own hands one over through
/// [`Glass::glass_effect`].
pub fn of(corner_radius: f32, style: SurfaceStyle, child: impl IntoElement) -> Surface {
    Surface {
        corners: uniform(corner_radius),
        glass: Look::Shipped(style),
        tint: None,
        child: child.into_any_element(),
    }
}

/// The same, on [`Theme::menu_style`] — what the popover layers mount on, so an
/// app moves every menu between frost and glass by moving one token.
pub fn popover(corner_radius: f32, child: impl IntoElement) -> Surface {
    Surface {
        corners: uniform(corner_radius),
        glass: Look::Popover,
        tint: None,
        child: child.into_any_element(),
    }
}

/// One radius on all four corners.
fn uniform(radius: f32) -> Corners<AbsoluteLength> {
    let radius = AbsoluteLength::from(px(radius));
    Corners {
        top_left: radius,
        top_right: radius,
        bottom_right: radius,
        bottom_left: radius,
    }
}

/// Where a [`Material`]'s numbers come from. The popover layers carry no theme
/// of their own, so they name a look and it resolves against whichever theme
/// paints them; a caller tuning its own glass hands the numbers over instead.
#[derive(Clone, Copy)]
enum Look {
    /// Whatever [`Theme::popover_surface`] says — the popover layers' choice.
    Popover,
    Shipped(SurfaceStyle),
    Tuned(Tokens),
}

impl Look {
    fn tokens(self, theme: &Theme) -> Tokens {
        match self {
            Look::Popover => Tokens::of(theme, theme.popover_surface),
            Look::Shipped(style) => Tokens::of(theme, style),
            Look::Tuned(tokens) => tokens,
        }
    }
}

/// The glass numbers, read off the theme the way every other widget reads its
/// colours. Not parameters: a caller who wants different glass hands over a
/// different theme.
#[derive(Clone, Copy)]
struct Tokens {
    spec: SurfaceSpec,
    magnify: f32,
    dispersion: f32,
}

impl Tokens {
    fn of(theme: &Theme, style: SurfaceStyle) -> Self {
        Self {
            spec: style.spec(theme),
            magnify: theme.glass_magnify,
            dispersion: theme.glass_dispersion,
        }
    }
}

/// The backdrop surfaces, on any element carrying a corner radius.
pub trait Surfaced: Styled + IntoElement + Sized {
    /// Paint this card as liquid glass — SwiftUI's `Glass.clear`: a refracting
    /// bevel at the rim, a lit edge, and [`Theme::glass_clear`]'s dimming.
    ///
    /// Blur, bevel and tint all come off [`Theme`], and the shape off the
    /// card's own rounding: the lens dies if any of the three is wrong.
    ///
    /// It clears the card's `bg`, since the lens paints the fill. Where the
    /// lens cannot run — every renderer but macOS Metal — it paints the frosted
    /// tint instead, so the surface is never invisible.
    fn surface(mut self, theme: &Theme, style: SurfaceStyle) -> Surface {
        let corners = corners_of(&mut self);
        // The lens paints the fill, so the card's own is dropped here rather
        // than at the call site: painting both is what buries the lens, and a
        // caller who has to remember that is a caller who will forget.
        self.style().background = None;
        Surface {
            corners,
            glass: Look::Tuned(Tokens::of(theme, style)),
            tint: None,
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

impl<E: Styled + IntoElement> Surfaced for E {}

pub struct Surface {
    /// The glass's own tint, when the caller wants one. `None` is
    /// [`Theme::glass_clear`]'s neutral lift.
    tint: Option<gpui::Hsla>,
    /// Held unresolved: a rem-rounded card only becomes pixels against the
    /// window's rem size, which paint is the first place to have.
    corners: Corners<AbsoluteLength>,
    /// Which look to paint, and where its numbers come from.
    glass: Look,
    child: AnyElement,
}

/// Whether this build has the backdrop-blur primitive behind it. Metal reads it
/// off the scene, and so does wgpu now that the fork implements it there —
/// which is why this tracks the gpui in use rather than the platform alone.
const LENSED: bool = cfg!(any(target_os = "macos", target_family = "wasm"));

/// Whether [`Glass::glass_effect`] will actually refract here, or fall back to
/// the flat backdrop tint. Capability and choice both: the primitive is macOS
/// Metal's and wgpu's, and components with glass off paint no lens.
pub fn lensed(theme: &Theme) -> bool {
    LENSED && theme.glass
}

impl Surface {
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

    /// The card's rounding in pixels, which only a rem size resolves — clamped
    /// to the box, since gpui reads a radius past half of it as a sharp corner.
    fn corners(&self, bounds: Bounds<Pixels>, rem: Pixels) -> Corners<Pixels> {
        Corners {
            top_left: self.corners.top_left.to_pixels(rem),
            top_right: self.corners.top_right.to_pixels(rem),
            bottom_right: self.corners.bottom_right.to_pixels(rem),
            bottom_left: self.corners.bottom_left.to_pixels(rem),
        }
        .clamp_radii_for_quad_size(bounds.size)
    }
}

impl Element for Surface {
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
        let glass = self.glass.tokens(theme);
        let corners = self.corners(bounds, window.rem_size());
        // The backdrop-blur primitive is macOS Metal's and wgpu's alone. The
        // surface's fill lives inside it, so anywhere it will not run the fill
        // is painted here — the look's own tint, so the card degrades to a
        // surface with the page showing through rather than to an opaque slab.
        if !lensed(theme) {
            let tint = self.tint.unwrap_or(glass.spec.tint);
            window.paint_quad(fill(bounds, tint).corner_radii(corners));
        }
        if theme.glass {
            let extent = f32::from(bounds.size.width.min(bounds.size.height));
            let effect = gpui::GlassEffect {
                blur_radius: px(glass.spec.blur),
                // A length, not a share of the box — but two rims cannot meet
                // in the middle of a small one.
                lens: px(glass.spec.rim.min(extent / 2.0)),
                // A length, like the lens: the measured drag is one curve of
                // distance from the rim, the same on a 96pt box and a 320pt
                // one. Held under half the box, where two reaches would cross.
                reach: px(glass.spec.reach.min(extent / 2.0)),
                gain: glass.spec.gain,
                saturation: glass.spec.saturation,
                magnify: glass.magnify,
                dispersion: glass.dispersion,
                tint: self.tint.unwrap_or(glass.spec.tint),
                edge: glass.spec.edge,
                edge_width: px(glass.spec.edge_width),
                edge_aa: px(glass.spec.edge_aa),
            };
            window.paint_layer(bounds, |window| {
                window.paint_backdrop_blur(bounds, corners, effect);
                // After the blur, never before: the blur samples what is
                // beneath it, so a shadow painted first is one the surface
                // shows through itself. Cut to outside the shape, so it lands
                // on the page and nowhere else.
                if glass.spec.shadow {
                    window.paint_drop_shadows_outside(bounds, corners, &theme::surface_shadows());
                }
                self.child.paint(window, cx);
            });
        } else {
            self.child.paint(window, cx);
        }
    }
}

impl IntoElement for Surface {
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
