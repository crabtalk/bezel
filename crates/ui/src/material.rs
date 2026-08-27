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

use gpui::{
    AbsoluteLength, AnyElement, App, Bounds, Corners, Element, GlobalElementId, InspectorElementId,
    IntoElement, LayoutId, Pixels, Styled, Window, px,
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
        child: child.into_any_element(),
    }
}

/// Frost this card at the corner radius it already carries.
///
/// The radius comes off the element's own style, so a caller chaining its own
/// rounding — `card.rounded(px(4.0)).material(MENU_BLUR)` — gets a blur cut to
/// the corners it just asked for.
pub trait Frosted: Styled + IntoElement + Sized {
    fn material(mut self, blur_radius: f32) -> Material {
        let square = AbsoluteLength::from(px(0.0));
        let radii = &self.style().corner_radii;
        let corners = Corners {
            top_left: radii.top_left.unwrap_or(square),
            top_right: radii.top_right.unwrap_or(square),
            bottom_right: radii.bottom_right.unwrap_or(square),
            bottom_left: radii.bottom_left.unwrap_or(square),
        };
        Material {
            corners,
            blur_radius,
            child: self.into_any_element(),
        }
    }
}

impl<E: Styled + IntoElement> Frosted for E {}

pub struct Material {
    /// Held unresolved: a rem-rounded card only becomes pixels against the
    /// window's rem size, which paint is the first place to have.
    corners: Corners<AbsoluteLength>,
    blur_radius: f32,
    child: AnyElement,
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
        if Theme::of(cx).is_glass() {
            let rem = window.rem_size();
            let corners = Corners {
                top_left: self.corners.top_left.to_pixels(rem),
                top_right: self.corners.top_right.to_pixels(rem),
                bottom_right: self.corners.bottom_right.to_pixels(rem),
                bottom_left: self.corners.bottom_left.to_pixels(rem),
            };
            window.paint_layer(bounds, |window| {
                window.paint_backdrop_blur(bounds, corners, px(self.blur_radius));
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
