//! Where the title, legends, plot and axes go at the chart's size: Vega's
//! layout, its `pad` autosize kept inside the host's size. The title and
//! legends take their room from the chart's edges first, and the plot
//! takes what its axes leave of the rest.

use crate::{
    model::{Align, Dir, Lettering, Side},
    plan,
    plan::{Axis, Face, Ink, Label, Metrics, Paint, Rect, look},
};
use gpui::{FontStyle, FontWeight, SharedString};

/// Vega-Lite's `padding` around the whole chart.
const PADDING: f32 = 5.0;
/// Vega's guide titles, chart title and subtitle against its 10-pixel
/// guide labels, which the host's text size stands for.
pub(super) const GUIDE_TITLE: f32 = 1.1;
pub(super) const CHART_TITLE: f32 = 1.3;
pub(super) const SUBTITLE: f32 = 1.2;
/// How many times the plot is fit to what its axes take of the room.
const PASSES: usize = 3;

pub(super) struct Layout<'a> {
    pub(super) metrics: Metrics,
    measure: &'a mut dyn FnMut(&str, &Face) -> f32,
    /// What the title and legends leave for the plot and its axes.
    pub(super) inner: Rect,
}

impl<'a> Layout<'a> {
    pub(super) fn new(
        size: [f32; 2],
        metrics: Metrics,
        measure: &'a mut dyn FnMut(&str, &Face) -> f32,
    ) -> Self {
        let inner = Rect {
            x: PADDING,
            y: PADDING,
            w: (size[0] - 2.0 * PADDING).max(0.0),
            h: (size[1] - 2.0 * PADDING).max(0.0),
        };
        Self {
            metrics,
            measure,
            inner,
        }
    }

    /// `lettering`'s face: `scale` times the host's size, and bold where
    /// `bold`, unless it says otherwise.
    pub(super) fn face(&self, lettering: &Lettering, scale: f32, bold: bool) -> Face {
        Face {
            family: lettering.font.clone(),
            size: lettering.size.unwrap_or(self.metrics.size * scale),
            weight: lettering.weight.unwrap_or(match bold {
                true => FontWeight::BOLD,
                false => FontWeight::NORMAL,
            }),
            style: lettering.style.unwrap_or(FontStyle::Normal),
        }
    }

    /// `text` set as `lettering` says, else in `face`'s defaults and `ink`,
    /// its top-left at the origin and each line cut short at `limit` pixels,
    /// `0` for none.
    pub(super) fn label(
        &mut self,
        text: &str,
        lettering: &Lettering,
        (scale, bold): (f32, bool),
        ink: Ink,
        limit: f32,
    ) -> Label {
        let face = self.face(lettering, scale, bold);
        let line = lettering
            .line_height
            .unwrap_or(face.size * self.metrics.line / self.metrics.size.max(1.0));
        let limit = lettering.limit.unwrap_or(limit);
        let mut width: f32 = 0.0;
        let lines: Vec<String> = text
            .split('\n')
            .map(|content| {
                let content = match limit > 0.0 {
                    true => Dir::Ltr
                        .truncated(content, limit, look::ELLIPSIS, |t| (self.measure)(t, &face)),
                    false => content.to_string(),
                };
                width = width.max((self.measure)(&content, &face));
                content
            })
            .collect();
        Label {
            text: SharedString::from(lines.join("\n")),
            at: [0.0, 0.0],
            width,
            line,
            paint: Paint {
                ink: lettering.color.map_or(ink, Ink::Fixed),
                alpha: lettering.opacity.unwrap_or(1.0),
            },
            face,
            align: lettering.align.unwrap_or(Align::Left),
        }
    }

    /// Takes `extent` pixels of the inner room from its `side`.
    pub(super) fn reserve(&mut self, side: Side, extent: f32) {
        let extent = extent.max(0.0);
        let inner = &mut self.inner;
        match side {
            Side::Top => {
                inner.y += extent;
                inner.h -= extent;
            }
            Side::Bottom => inner.h -= extent,
            Side::Left => {
                inner.x += extent;
                inner.w -= extent;
            }
            Side::Right => inner.w -= extent,
        }
        inner.w = inner.w.max(0.0);
        inner.h = inner.h.max(0.0);
    }

    /// The plot inside the inner room, less what the axes `place` draws
    /// at a plot take past it, and what `place` gives there. `None` where
    /// `place` gives none.
    pub(super) fn fit<T>(
        &mut self,
        mut place: impl FnMut(&mut Self, Rect) -> Option<(T, Vec<Axis>)>,
    ) -> Option<(Rect, T, Vec<Axis>)> {
        // Top, right, bottom and left.
        let mut margins = [0.0f32; 4];
        let mut out = None;
        for _ in 0..PASSES {
            let inner = self.inner;
            let plot = Rect {
                x: inner.x + margins[3],
                y: inner.y + margins[0],
                w: (inner.w - margins[1] - margins[3]).max(plan::MIN_PLOT),
                h: (inner.h - margins[0] - margins[2]).max(plan::MIN_PLOT),
            };
            let (placed, axes) = place(self, plot)?;
            let taken = match axes.iter().filter_map(Axis::bounds).reduce(Rect::union) {
                Some(b) => [
                    plot.y - b.y,
                    b.right() - plot.right(),
                    b.bottom() - plot.bottom(),
                    plot.x - b.x,
                ]
                .map(|m| m.max(0.0).ceil()),
                None => [0.0; 4],
            };
            out = Some((plot, placed, axes));
            if taken.iter().zip(margins).all(|(a, b)| (a - b).abs() < 0.5) {
                break;
            }
            margins = taken;
        }
        out
    }
}
