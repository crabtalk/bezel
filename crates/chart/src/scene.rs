//! A Vega [`Drawing`] painted with gpui: its background, then its marks,
//! group by group, at the origin Vega gives the root group.
//!
//! Groups, rects, rules and text paint. Text paints upright whatever its
//! angle, in the host's default font where it names `sans-serif`.

use crate::{color, paint::polyline};
use d3::color::Rgb;
use gpui::{
    App, Bounds, ContentMask, Element, ElementId, Font, FontWeight, GlobalElementId, Hsla,
    InspectorElementId, IntoElement, LayoutId, Pixels, Point, Refineable, SharedString, Style,
    StyleRefinement, Styled, TextAlign, TextRun, Window, fill, point, px, size,
};
use std::rc::Rc;
use vega::{
    Drawing,
    scene::{Item, Mark},
    text,
    value::Value,
};

/// `drawing` as an element, its own size unless styled.
pub fn scene(id: impl Into<ElementId>, drawing: Drawing) -> SceneView {
    let [top, right, bottom, left] = drawing.padding;
    let mut style = StyleRefinement::default();
    style.size.width = Some(px((drawing.width + left + right) as f32).into());
    style.size.height = Some(px((drawing.height + top + bottom) as f32).into());
    SceneView {
        id: id.into(),
        drawing: Rc::new(drawing),
        style,
    }
}

pub struct SceneView {
    id: ElementId,
    drawing: Rc<Drawing>,
    style: StyleRefinement,
}

impl Styled for SceneView {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl IntoElement for SceneView {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for SceneView {
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
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, ()) {
        let mut style = Style::default();
        style.refine(&self.style);
        (window.request_layout(style, [], cx), ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _request_layout: &mut (),
        _window: &mut Window,
        _cx: &mut App,
    ) {
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut (),
        _prepaint: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        let drawing = self.drawing.clone();
        if let Some(background) = drawing.background.as_deref().and_then(css) {
            window.paint_quad(fill(bounds, background));
        }
        let [top, _, _, left] = drawing.padding;
        let origin = bounds.origin
            + point(
                px((left + drawing.origin[0]) as f32),
                px((top + drawing.origin[1]) as f32),
            );
        let font = window.text_style().font();
        window.with_content_mask(
            Some(ContentMask {
                bounds,
                ..ContentMask::default()
            }),
            |window| {
                mark(&drawing.scene, origin, &font, window, cx);
            },
        );
    }
}

/// A CSS colour as gpui paints it; `None` for one d3-color cannot read.
fn css(text: &str) -> Option<Hsla> {
    text.parse::<Rgb>().ok().map(color::hsla)
}

/// The item's `name` as a number, `default` where it is unset.
fn number(item: &Item, name: &str, default: f64) -> f64 {
    item.number(name).unwrap_or(default)
}

/// The item's opacity times its `fillOpacity` or `strokeOpacity`.
fn alpha(item: &Item, channel: &str) -> f32 {
    (number(item, "opacity", 1.0) * number(item, channel, 1.0)) as f32
}

/// The item's `fill` or `stroke`, faded by its opacities; `None` where it
/// paints none.
fn paint(item: &Item, channel: &str) -> Option<Hsla> {
    let Value::String(text) = item.get(channel) else {
        return None;
    };
    let mut color = css(text)?;
    color.a *= alpha(
        item,
        match channel {
            "fill" => "fillOpacity",
            _ => "strokeOpacity",
        },
    );
    (color.a > 0.0).then_some(color)
}

fn at(origin: Point<Pixels>, x: f64, y: f64) -> Point<Pixels> {
    origin + point(px(x as f32), px(y as f32))
}

/// A mark's items, `origin` its group's.
fn mark(mark: &Mark, origin: Point<Pixels>, font: &Font, window: &mut Window, cx: &mut App) {
    for item in &mark.items {
        if number(item, "opacity", 1.0) <= 0.0 {
            continue;
        }
        match mark.marktype.as_str() {
            "group" => group(item, origin, font, window, cx),
            "rect" => rect(item, origin, window),
            "rule" => rule(item, origin, window),
            "text" => label(item, origin, font, window, cx),
            _ => {}
        }
    }
}

fn rect(item: &Item, origin: Point<Pixels>, window: &mut Window) {
    let (x, y) = (item.or_zero("x"), item.or_zero("y"));
    let (w, h) = (item.or_zero("width"), item.or_zero("height"));
    let bounds = Bounds::new(at(origin, x, y), size(px(w as f32), px(h as f32)));
    let mut quad = fill(
        bounds,
        paint(item, "fill").unwrap_or_else(Hsla::transparent_black),
    );
    if let Some(stroke) = paint(item, "stroke") {
        quad = quad
            .border_widths(px(number(item, "strokeWidth", 1.0) as f32))
            .border_color(stroke);
    }
    window.paint_quad(quad);
}

fn group(item: &Item, origin: Point<Pixels>, font: &Font, window: &mut Window, cx: &mut App) {
    rect(item, origin, window);
    let inner = at(origin, item.or_zero("x"), item.or_zero("y"));
    let clip = item.get("clip").truthy().then(|| {
        Bounds::new(
            inner,
            size(
                px(item.or_zero("width") as f32),
                px(item.or_zero("height") as f32),
            ),
        )
    });
    window.with_content_mask(
        clip.map(|bounds| ContentMask {
            bounds,
            ..ContentMask::default()
        }),
        |window| {
            for child in &item.items {
                mark(child, inner, font, window, cx);
            }
        },
    );
}

fn rule(item: &Item, origin: Point<Pixels>, window: &mut Window) {
    let Some(stroke) = paint(item, "stroke") else {
        return;
    };
    let (x1, y1) = (item.or_zero("x"), item.or_zero("y"));
    let (x2, y2) = (number(item, "x2", x1), number(item, "y2", y1));
    let point_at = |x: f64, y: f64| {
        [
            (origin.x.as_f32() + x as f32),
            (origin.y.as_f32() + y as f32),
        ]
    };
    let dash: Option<Vec<f32>> = match item.get("strokeDash") {
        Value::Array(dash) if !dash.is_empty() => {
            Some(dash.iter().map(|d| d.to_number() as f32).collect())
        }
        _ => None,
    };
    let width = number(item, "strokeWidth", 1.0) as f32;
    if let Some(path) = polyline(
        &[point_at(x1, y1), point_at(x2, y2)],
        width,
        dash.as_deref(),
    ) {
        window.paint_path(path, stroke);
    }
}

fn label(item: &Item, origin: Point<Pixels>, font: &Font, window: &mut Window, cx: &mut App) {
    let Some(color) = paint(item, "fill") else {
        return;
    };
    let content = match item.get("text") {
        Value::Undefined | Value::Null => return,
        Value::Array(lines) => lines
            .iter()
            .map(Value::to_string)
            .collect::<Vec<_>>()
            .join(" "),
        value => value.to_string(),
    };
    if content.is_empty() {
        return;
    }
    let font_size = text::font_size(item);
    let family = match item.get("font") {
        Value::String(family) if &**family != "sans-serif" => {
            SharedString::from(family.to_string())
        }
        _ => font.family.clone(),
    };
    let weight = match item.get("fontWeight") {
        Value::String(w) if &**w == "bold" => FontWeight::BOLD,
        Value::Number(w) => FontWeight(*w as f32),
        _ => font.weight,
    };
    let run = TextRun {
        len: content.len(),
        font: Font {
            family,
            weight,
            ..font.clone()
        },
        color,
        background_color: None,
        underline: None,
        strikethrough: None,
    };
    let line = window
        .text_system()
        .shape_line(content.into(), px(font_size as f32), &[run], None);
    let width = line.width().as_f32();
    // Vega sets the alphabetic baseline its `offset` from the anchor.
    let x = item.or_zero("x") + item.or_zero("dx");
    let y = item.or_zero("y") + item.or_zero("dy") + text::offset(item);
    let shift = match item.get("align") {
        Value::String(a) if &**a == "center" => width / 2.0,
        Value::String(a) if &**a == "right" => width,
        _ => 0.0,
    };
    let top_left = at(origin, x, y) - point(px(shift), line.ascent);
    let height = line.ascent + line.descent;
    line.paint(top_left, height, TextAlign::Left, None, window, cx)
        .ok();
}
