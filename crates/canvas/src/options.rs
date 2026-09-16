//! What the canvas is tuned by, what its paint measures, and the overlays a
//! node wears — in place of constants an app could not reach.
//!
//! ```ignore
//! CanvasView::new(doc, layout, cx).with_options(Options { max_zoom: 8.0, ..Options::default() })
//! ```
//!
//! [`Options`] is the editor's, and the tools read it too; [`Style`] and
//! [`Overlays`] are the view's.

use std::rc::Rc;

use gpui::{
    AnyElement, App, Bounds, PathBuilder, Pixels, Point, Window, div, fill, point, prelude::*, px,
    size,
};
use theme::Theme;

use crate::snap::{Axis, Guide};

/// What the canvas is tuned by.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Options {
    pub min_zoom: f32,
    pub max_zoom: f32,
    /// One chord's zoom.
    pub zoom_step: f32,
    /// Zoom per pixel of a modified wheel.
    pub wheel_zoom: f32,
    /// How far a press travels before it is a drag, in screen pixels.
    pub drag_threshold: f32,
    /// One `shift`-arrow, in canvas units.
    pub nudge: i64,
    /// How far a duplicate sits from what it copies, in canvas units.
    pub duplicate_offset: i64,
    /// Undo steps kept.
    pub history: usize,
    /// The smallest box a corner pulls a node to, in canvas units.
    pub min_size: (i64, i64),
    /// The room `fit` leaves around what it shows, in screen pixels.
    pub fit_padding: f32,
    /// The closest `zoom_to_selection` comes.
    pub selection_zoom: f32,
    /// How near the view's edge a node brought into view sits, in screen
    /// pixels.
    pub reveal_margin: f32,
    /// How near a dragged box's line comes to another's before it catches, in
    /// screen pixels.
    pub snap_reach: f32,
    /// The most one frame of drift may travel, in seconds, however late it
    /// ran.
    pub drift_step: f32,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            min_zoom: 0.25,
            max_zoom: 4.0,
            zoom_step: 1.25,
            wheel_zoom: 0.01,
            drag_threshold: 3.0,
            nudge: 8,
            duplicate_offset: 24,
            history: 200,
            min_size: (60, 32),
            fit_padding: 32.0,
            selection_zoom: 2.0,
            reveal_margin: 24.0,
            snap_reach: 6.0,
            drift_step: 0.05,
        }
    }
}

/// What the canvas's own paint measures.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Style {
    /// How far the selection ring sits outside a node, in screen pixels.
    pub ring: f32,
    /// A handle's box, in screen pixels.
    pub handle: f32,
    /// Arrowhead length, in canvas units.
    pub arrow: f32,
    /// The room an edge label is centred in, in canvas units.
    pub label: (f32, f32),
    /// How much of a connector a drop would cut still shows.
    pub cut: f32,
    /// The accent wash inside a node a drop would land on.
    pub target_wash: f32,
    /// The accent wash inside a marquee.
    pub marquee_wash: f32,
    /// Below this zoom a node paints as its box, its content unread.
    pub far_zoom: f32,
    /// The closest grid dots come, in screen pixels; a denser grid skips rows.
    pub dot_spacing: f32,
    /// A grid dot, in screen pixels.
    pub dot: f32,
}

impl Default for Style {
    fn default() -> Self {
        Self {
            ring: 3.0,
            handle: 8.0,
            arrow: 8.0,
            label: (240.0, 32.0),
            cut: 0.25,
            target_wash: 0.12,
            marquee_wash: 0.08,
            far_zoom: 0.4,
            dot_spacing: 12.0,
            dot: 1.5,
        }
    }
}

/// What an overlay is painted over: the box it marks, in screen pixels, and
/// the zoom it is painted at.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Mark {
    pub width: f32,
    pub height: f32,
    pub zoom: f32,
    pub style: Style,
}

/// One overlay, painted to fill the box it marks.
pub type Painter = Rc<dyn Fn(&Mark, &mut Window, &mut App) -> AnyElement>;

/// Where the canvas is painting: the view's corner in the window, what it is
/// panned and zoomed to, how big it is, and what its paint measures by.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Frame {
    pub origin: Point<f32>,
    pub pan: Point<f32>,
    pub zoom: f32,
    pub width: f32,
    pub height: f32,
    pub style: Style,
}

impl Frame {
    /// A canvas point, in window pixels.
    pub fn screen(&self, at: Point<f32>) -> Point<f32> {
        point(
            self.origin.x + self.pan.x + at.x * self.zoom,
            self.origin.y + self.pan.y + at.y * self.zoom,
        )
    }
}

/// What is painted behind the nodes, given the grid the snap settles on, in
/// canvas units.
pub type Grid = Rc<dyn Fn(&Frame, i64, &mut Window, &mut App)>;

/// What the lines a drag caught on are painted as.
pub type Guides = Rc<dyn Fn(&Frame, &[Guide], &mut Window, &mut App)>;

/// What the canvas paints beside what the kinds do. Each is replaceable; the
/// marquee and the connector belong to the tools that draw them.
#[derive(Clone)]
pub struct Overlays {
    /// Around a node that is picked.
    pub ring: Painter,
    /// Over a node a drop would land on.
    pub drop: Painter,
    /// A handle its kind declared.
    pub handle: Painter,
    /// A node too far out for its content to be read.
    pub placeholder: Painter,
    /// Behind every node, when the snap settles on a grid.
    pub grid: Grid,
    /// The lines a drag caught on.
    pub guides: Guides,
}

impl Overlays {
    pub fn new() -> Self {
        Self {
            ring: Rc::new(ring),
            drop: Rc::new(drop_wash),
            handle: Rc::new(handle),
            placeholder: Rc::new(placeholder),
            grid: Rc::new(grid),
            guides: Rc::new(guides),
        }
    }
}

impl Default for Overlays {
    fn default() -> Self {
        Self::new()
    }
}

/// The corners a box and the ring around it are rounded to, in canvas units.
const RADIUS: f32 = crate::kind::RADIUS;

/// An accent ring, outside the box it marks.
pub fn ring(mark: &Mark, _: &mut Window, cx: &mut App) -> AnyElement {
    let out = mark.style.ring;
    div()
        .absolute()
        .top(px(-out))
        .left(px(-out))
        .right(px(-out))
        .bottom(px(-out))
        .rounded(px(RADIUS * mark.zoom + out))
        .border_2()
        .border_color(Theme::of(cx).accent)
        .into_any_element()
}

/// An accent wash over the whole box.
pub fn drop_wash(mark: &Mark, _: &mut Window, cx: &mut App) -> AnyElement {
    div()
        .absolute()
        .top_0()
        .left_0()
        .size_full()
        .rounded(px(RADIUS * mark.zoom))
        .bg(Theme::of(cx).accent.opacity(mark.style.target_wash))
        .into_any_element()
}

/// A small accent box, which the view places where the handle sits.
pub fn handle(mark: &Mark, _: &mut Window, cx: &mut App) -> AnyElement {
    let theme = Theme::of(cx);
    div()
        .size(px(mark.style.handle))
        .border_1()
        .border_color(theme.accent)
        .bg(theme.surface_card)
        .into_any_element()
}

/// Dots at every `step`, thinned out until they are no closer than the style
/// asks for. Nonpositive steps paint nothing.
pub fn grid(frame: &Frame, step: i64, window: &mut Window, cx: &mut App) {
    if step <= 0 {
        return;
    }
    let ink = Theme::of(cx).border;
    let dot = frame.style.dot;
    let mut gap = step as f32 * frame.zoom;
    while gap < frame.style.dot_spacing {
        gap *= 2.0;
    }
    let mut y = frame.pan.y.rem_euclid(gap);
    while y < frame.height {
        let mut x = frame.pan.x.rem_euclid(gap);
        while x < frame.width {
            let at = point(
                px(frame.origin.x + x - dot / 2.0),
                px(frame.origin.y + y - dot / 2.0),
            );
            window.paint_quad(fill(Bounds::new(at, size(px(dot), px(dot))), ink));
            x += gap;
        }
        y += gap;
    }
}

/// A hairline along each line a drag caught on.
pub fn guides(frame: &Frame, caught: &[Guide], window: &mut Window, cx: &mut App) {
    let accent = Theme::of(cx).accent;
    for guide in caught {
        let (at, from, to) = (guide.at as f32, guide.from as f32, guide.to as f32);
        let (a, b) = match guide.axis {
            Axis::X => (point(at, from), point(at, to)),
            Axis::Y => (point(from, at), point(to, at)),
        };
        let mut path = PathBuilder::stroke(px(1.0));
        path.move_to(pt(frame.screen(a)));
        path.line_to(pt(frame.screen(b)));
        if let Ok(path) = path.build() {
            window.paint_path(path, accent);
        }
    }
}

fn pt(at: Point<f32>) -> Point<Pixels> {
    point(px(at.x), px(at.y))
}

/// A plain box, for a node too small to read.
pub fn placeholder(mark: &Mark, _: &mut Window, cx: &mut App) -> AnyElement {
    let theme = Theme::of(cx);
    div()
        .size_full()
        .rounded(px(RADIUS * mark.zoom))
        .border_1()
        .border_color(theme.border)
        .bg(theme.surface_card)
        .into_any_element()
}
