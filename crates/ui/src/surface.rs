//! Floating surfaces with platform-owned material treatment.

use gpui::{
    self, AbsoluteLength, AnyElement, App, Bounds, Corners, Div, Element, ElementId,
    GlobalElementId, InspectorElementId, InteractiveElement, Interactivity, IntoElement, LayoutId,
    ParentElement, Pixels, RenderOnce, StatefulInteractiveElement, StyleRefinement, Styled, Window,
    native_material::{MaterialState, MaterialStyle},
};
use theme::{Appearance, SurfaceStyle, Theme};

/// Build a styled card inside its material's content appearance.
pub fn of<E: Styled + IntoElement>(
    id: impl Into<ElementId>,
    style: SurfaceStyle,
    build: impl FnOnce(&Theme, &mut Window, &mut App) -> E + 'static,
) -> Surface {
    Surface::styled(id.into(), Look::Shipped(style), build)
}

/// Build a carried item using the theme's carried-surface style.
pub fn carried<E: Styled + IntoElement>(
    id: impl Into<ElementId>,
    build: impl FnOnce(&Theme, &mut Window, &mut App) -> E + 'static,
) -> Surface {
    Surface::styled(id.into(), Look::Carried, build)
}

/// Build popover content at the caller's corner radius.
pub fn popover<E: IntoElement>(
    id: impl Into<ElementId>,
    corner_radius: f32,
    build: impl FnOnce(&Theme, &mut Window, &mut App) -> E + 'static,
) -> Surface {
    Surface::new(id.into(), Look::Popover, move |theme, window, cx| {
        (
            uniform(corner_radius),
            build(theme, window, cx).into_any_element(),
        )
    })
}

/// Group related glass surfaces without merging their shapes.
pub fn group<E: IntoElement>(
    id: impl Into<ElementId>,
    build: impl FnOnce(&Theme, &mut Window, &mut App) -> E + 'static,
) -> Surface {
    Surface::new(id.into(), Look::Group, move |theme, window, cx| {
        (uniform(0.), build(theme, window, cx).into_any_element())
    })
}

/// A selection material whose bounds may move independently of its foreground.
pub fn selection(
    id: impl Into<ElementId>,
    child: impl IntoElement,
    place: impl FnMut(Bounds<Pixels>, &mut Window, &mut App) -> Option<(Bounds<Pixels>, Pixels)>
    + 'static,
) -> Surface {
    let child = child.into_any_element();
    let mut surface = Surface::new(id.into(), Look::Selection, move |_, _, _| {
        (uniform(0.), child)
    });
    surface.selection = Some(Box::new(place));
    surface
}

/// A control whose selected background uses the platform material.
#[derive(gpui::IntoElement)]
pub struct Control {
    inner: Div,
    selected: bool,
}

impl Control {
    pub fn new(inner: Div, selected: bool) -> Self {
        Self { inner, selected }
    }

    pub fn id(mut self, id: impl Into<ElementId>) -> Self {
        self.inner.interactivity().element_id = Some(id.into());
        self
    }

    pub fn material(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }
}

impl From<Div> for Control {
    fn from(inner: Div) -> Self {
        Self::new(inner, false)
    }
}

impl Styled for Control {
    fn style(&mut self) -> &mut StyleRefinement {
        self.inner.style()
    }
}

impl ParentElement for Control {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.inner.extend(elements);
    }
}

impl InteractiveElement for Control {
    fn interactivity(&mut self) -> &mut Interactivity {
        self.inner.interactivity()
    }
}

impl StatefulInteractiveElement for Control {}

impl RenderOnce for Control {
    fn render(mut self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        if !self.selected || !window.native_surfaces.materials || !Theme::of(cx).glass {
            return self.inner.into_any_element();
        }
        let id = Element::id(&self.inner).unwrap_or_else(|| "selection-material".into());
        let corners = corners_of(&mut self.inner);
        self.inner.style().background = None;
        self.inner.style().box_shadow = Some(Vec::new());
        selection(id, self.inner, move |bounds, window, _| {
            let rem = window.rem_size();
            let radius = [
                corners.top_left,
                corners.top_right,
                corners.bottom_right,
                corners.bottom_left,
            ]
            .into_iter()
            .map(|radius| radius.to_pixels(rem))
            .max()
            .unwrap_or_default();
            Some((bounds, radius))
        })
        .into_any_element()
    }
}

fn uniform(radius: f32) -> Corners<AbsoluteLength> {
    let radius = AbsoluteLength::from(gpui::px(radius));
    Corners {
        top_left: radius,
        top_right: radius,
        bottom_right: radius,
        bottom_left: radius,
    }
}

fn corners_of(styled: &mut impl Styled) -> Corners<AbsoluteLength> {
    let square = AbsoluteLength::from(gpui::px(0.));
    let radii = &styled.style().corner_radii;
    Corners {
        top_left: radii.top_left.unwrap_or(square),
        top_right: radii.top_right.unwrap_or(square),
        bottom_right: radii.bottom_right.unwrap_or(square),
        bottom_left: radii.bottom_left.unwrap_or(square),
    }
}

#[derive(Clone, Copy)]
enum Look {
    Popover,
    Carried,
    Shipped(SurfaceStyle),
    Group,
    Selection,
}

type Selection =
    dyn FnMut(Bounds<Pixels>, &mut Window, &mut App) -> Option<(Bounds<Pixels>, Pixels)>;

type Content = dyn FnOnce(&Theme, &mut Window, &mut App) -> (Corners<AbsoluteLength>, AnyElement);

#[cfg(not(target_os = "macos"))]
struct Optics {
    spec: theme::SurfaceSpec,
    magnify: f32,
    dispersion: f32,
}

/// A keyed material host and its lazy content.
pub struct Surface {
    id: ElementId,
    tint: Option<gpui::Hsla>,
    corners: Corners<AbsoluteLength>,
    look: Look,
    build: Option<Box<Content>>,
    child: Option<AnyElement>,
    material: MaterialState,
    selection: Option<Box<Selection>>,
    #[cfg(not(target_os = "macos"))]
    optics: Option<Optics>,
}

/// Whether material surfaces are enabled for this build and theme.
pub fn lensed(theme: &Theme) -> bool {
    theme::LENSED && theme.glass
}

impl Surface {
    fn styled<E: Styled + IntoElement>(
        id: ElementId,
        look: Look,
        build: impl FnOnce(&Theme, &mut Window, &mut App) -> E + 'static,
    ) -> Self {
        Self::new(id, look, move |theme, window, cx| {
            let mut child = build(theme, window, cx);
            let corners = corners_of(&mut child);
            child.style().background = None;
            (corners, child.into_any_element())
        })
    }

    fn new(
        id: ElementId,
        look: Look,
        build: impl FnOnce(&Theme, &mut Window, &mut App) -> (Corners<AbsoluteLength>, AnyElement)
        + 'static,
    ) -> Self {
        Self {
            id,
            tint: None,
            corners: uniform(0.),
            look,
            build: Some(Box::new(build)),
            child: None,
            material: MaterialState::default(),
            selection: None,
            #[cfg(not(target_os = "macos"))]
            optics: None,
        }
    }

    /// Override optical parameters on renderer-backed platforms.
    #[cfg(not(target_os = "macos"))]
    pub fn optics(mut self, theme: &Theme, style: SurfaceStyle) -> Self {
        self.optics = Some(Optics {
            spec: style.spec(theme),
            magnify: theme.glass_magnify,
            dispersion: theme.glass_dispersion,
        });
        self
    }

    /// Tint the native glass, or the renderer's glass on other platforms.
    pub fn tint(mut self, color: gpui::Hsla) -> Self {
        self.tint = Some(color);
        self
    }

    fn appearance(&self, cx: &App) -> Appearance {
        if !Theme::of(cx).glass {
            return Theme::of(cx).appearance;
        }
        self.material
            .appearance()
            .map(Appearance::from_window)
            .unwrap_or_else(|| Theme::of(cx).appearance)
    }

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

    fn id(&self) -> Option<ElementId> {
        Some(self.id.clone())
    }
    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        id: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, ()) {
        let id = id.expect("surface identity");
        self.material = window.with_element_state(id, |state: Option<MaterialState>, _| {
            let state = state.unwrap_or_default();
            (state.clone(), state)
        });
        Theme::with_appearance(self.appearance(cx), cx, |cx| {
            let theme = Theme::of(cx).clone();
            let (corners, child) =
                self.build.take().expect("surface content already built")(&theme, window, cx);
            self.corners = corners;
            self.child = Some(child);
            (self.child.as_mut().unwrap().request_layout(window, cx), ())
        })
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        if !matches!(self.look, Look::Group | Look::Selection) {
            crate::cover::record(bounds, window, cx);
        }
        Theme::with_appearance(self.appearance(cx), cx, |cx| {
            self.child.as_mut().unwrap().prepaint(window, cx)
        });
    }

    fn paint(
        &mut self,
        id: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut (),
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        Theme::with_appearance(self.appearance(cx), cx, |cx| {
            let theme = Theme::of(cx).clone();
            let corners = self.corners(bounds, window.rem_size());
            let style = match self.look {
                Look::Popover => Some(theme.popover_surface),
                Look::Carried => Some(theme.carried_surface),
                Look::Shipped(style) => Some(style),
                Look::Group | Look::Selection => None,
            };
            if matches!(self.look, Look::Selection) {
                let placed = self.selection.as_mut().unwrap()(bounds, window, cx);
                if window.native_surfaces.materials
                    && theme.glass
                    && let Some((placed, radius)) = placed
                {
                    let material = MaterialStyle::Glass {
                        clear: false,
                        tint: self.tint,
                        corner_radius: radius,
                        interactive: true,
                    };
                    window
                        .paint_native_material(
                            id.expect("surface identity"),
                            &self.material,
                            material,
                            placed,
                            |_| {},
                        )
                        .expect("native material capability changed during paint");
                    window.with_content_mask(Some(gpui::ContentMask::new(bounds)), |window| {
                        self.child.as_mut().unwrap().paint(window, cx);
                    });
                    return;
                }
                self.child.as_mut().unwrap().paint(window, cx);
                return;
            }
            if window.native_surfaces.materials && theme.glass {
                let radius = corners
                    .top_left
                    .max(corners.top_right)
                    .max(corners.bottom_right)
                    .max(corners.bottom_left);
                let material = match style {
                    Some(SurfaceStyle::Glass(glass)) => MaterialStyle::Glass {
                        clear: glass == theme::Glass::Clear,
                        tint: self.tint,
                        corner_radius: radius,
                        interactive: false,
                    },
                    Some(SurfaceStyle::Material(_)) => MaterialStyle::Popover {
                        corner_radius: radius,
                    },
                    None => MaterialStyle::Group {
                        spacing: gpui::px(0.),
                    },
                };
                window
                    .paint_native_material(
                        id.expect("surface identity"),
                        &self.material,
                        material,
                        bounds,
                        |window| self.child.as_mut().unwrap().paint(window, cx),
                    )
                    .expect("native material capability changed during paint");
                return;
            }
            let Some(style) = style else {
                self.child.as_mut().unwrap().paint(window, cx);
                return;
            };
            let spec = style.spec(&theme);
            #[cfg(not(target_os = "macos"))]
            let spec = self.optics.as_ref().map_or(spec, |optics| optics.spec);
            let tint = self.tint.unwrap_or(spec.tint);
            #[cfg(not(target_os = "macos"))]
            if lensed(&theme) {
                let extent = f32::from(bounds.size.width.min(bounds.size.height));
                let effect = gpui::GlassEffect {
                    blur_radius: gpui::px(spec.blur),
                    lens: gpui::px(spec.rim.min(extent / 2.)),
                    reach: gpui::px(spec.reach.min(extent / 2.)),
                    gain: spec.gain,
                    saturation: spec.saturation,
                    magnify: self
                        .optics
                        .as_ref()
                        .map_or(theme.glass_magnify, |optics| optics.magnify),
                    dispersion: self
                        .optics
                        .as_ref()
                        .map_or(theme.glass_dispersion, |optics| optics.dispersion),
                    tint,
                    edge: spec.edge,
                    edge_width: gpui::px(spec.edge_width),
                    edge_aa: gpui::px(spec.edge_aa),
                };
                window.paint_layer(|window| {
                    window.paint_backdrop_blur(bounds, corners, effect);
                    if spec.shadow {
                        window.paint_drop_shadows_outside(
                            bounds,
                            corners,
                            &theme::surface_shadows(),
                        );
                    }
                    self.child.as_mut().unwrap().paint(window, cx);
                });
                return;
            }
            window.paint_quad(
                gpui::fill(bounds, spec.flat(tint).unwrap_or(tint)).corner_radii(corners),
            );
            self.child.as_mut().unwrap().paint(window, cx);
        });
    }
}

/// Paint `child` in its own scene layer above preceding GPU primitives.
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
        _bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        _prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        window.paint_layer(|window| self.child.paint(window, cx));
    }
}
