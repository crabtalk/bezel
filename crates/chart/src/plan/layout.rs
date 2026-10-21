//! Where the header, legends, plot and axes go at the chart's size.

use gpui::SharedString;

use crate::plan::{self, Axis, Glyph, Label, Legend, Rect, Side, Swatch, Tick};

/// The chart's size, and how far down its title and legends reach.
pub(super) struct Layout<'a> {
    pub(super) size: [f32; 2],
    pub(super) line: f32,
    pub(super) top: f32,
    /// Kept under the bottom axis, for its title.
    pub(super) reserved: f32,
    pub(super) measure: &'a mut dyn FnMut(&str) -> f32,
}

impl Layout<'_> {
    pub(super) fn label(&mut self, text: SharedString, at: [f32; 2]) -> Label {
        let width = (self.measure)(&text);
        Label { text, at, width }
    }

    /// A full-width line of the header, at `x`.
    pub(super) fn line_at(&mut self, text: SharedString, x: f32) -> Label {
        let label = self.label(text, [x, self.top]);
        self.top += self.line + plan::GAP;
        label
    }

    /// `title`, then one entry per glyph and label, flowing left to right
    /// and wrapping, on lines of its own.
    pub(super) fn legend(
        &mut self,
        title: Option<SharedString>,
        entries: Vec<(Glyph, SharedString)>,
    ) -> Legend {
        let boxes: Vec<[f32; 2]> = entries.iter().map(|(glyph, _)| glyph.extent()).collect();
        let row = boxes.iter().map(|[_, h]| *h).fold(self.line, f32::max);
        let mut x = plan::PAD;
        let title = title.filter(|_| !entries.is_empty()).map(|text| {
            let label = self.label(text, [plan::PAD, self.top + (row - self.line) / 2.0]);
            x += label.width + plan::GAP;
            label
        });
        let mut swatches = Vec::new();
        for ((glyph, text), [w, h]) in entries.into_iter().zip(boxes) {
            let label = self.label(text, [0.0, 0.0]);
            let width = w + plan::GAP + label.width;
            if x > plan::PAD && x + width > self.size[0] - plan::PAD {
                x = plan::PAD;
                self.top += row;
            }
            swatches.push(Swatch {
                glyph,
                swatch: Rect {
                    x,
                    y: self.top + (row - h) / 2.0,
                    w,
                    h,
                },
                label: Label {
                    at: [x + w + plan::GAP, self.top + (row - self.line) / 2.0],
                    ..label
                },
            });
            x += width + plan::LEGEND_GAP;
        }
        if !swatches.is_empty() {
            self.top += row + plan::GAP;
        }
        Legend { title, swatches }
    }

    /// The plot's height once a bottom axis is taken out.
    pub(super) fn plot_height(&self) -> f32 {
        self.size[1] - self.top - plan::PAD - plan::GAP - self.line - self.reserved
    }

    /// The plot beside left labels `left` wide, over a bottom axis.
    pub(super) fn plot(&self, left: f32) -> Rect {
        let x = plan::PAD + left + plan::GAP;
        Rect {
            x,
            y: self.top,
            w: (self.size[0] - x - plan::PAD).max(plan::MIN_PLOT),
            h: self.plot_height().max(plan::MIN_PLOT),
        }
    }

    pub(super) fn measured(&mut self, labels: &[SharedString]) -> Vec<Label> {
        labels
            .iter()
            .map(|text| self.label(text.clone(), [0.0, 0.0]))
            .collect()
    }

    /// A left axis of measured `labels`, each centred on its `at` and
    /// right-aligned against `plot`. Past one to a line, every k-th.
    pub(super) fn left(&self, labels: Vec<Label>, at: &[f32], plot: Rect, grid: bool) -> Axis {
        let every = spacing(at).map_or(1, |step| (self.line / step).ceil().max(1.0) as usize);
        Axis {
            side: Side::Left,
            ticks: labels
                .into_iter()
                .zip(at)
                .enumerate()
                .map(|(index, (label, &y))| Tick {
                    at: y,
                    label: (index % every == 0).then(|| Label {
                        at: [plot.x - plan::GAP - label.width, y - self.line / 2.0],
                        ..label
                    }),
                })
                .collect(),
            grid,
            title: None,
        }
    }

    /// A bottom axis under `plot`, each label centred on its `at` and kept
    /// inside the chart. Past what fits side by side, every k-th.
    pub(super) fn bottom(
        &mut self,
        labels: &[SharedString],
        at: &[f32],
        plot: Rect,
        grid: bool,
    ) -> Axis {
        let measured = self.measured(labels);
        let every = spacing(at).map_or(1, |step| {
            ((widest(&measured) + plan::GAP) / step).ceil().max(1.0) as usize
        });
        let ticks = measured
            .into_iter()
            .zip(at)
            .enumerate()
            .map(|(index, (label, &x))| Tick {
                at: x,
                label: (index % every == 0).then(|| Label {
                    at: [
                        (x - label.width / 2.0).clamp(0.0, (self.size[0] - label.width).max(0.0)),
                        plot.bottom() + plan::GAP,
                    ],
                    ..label
                }),
            })
            .collect();
        Axis {
            side: Side::Bottom,
            ticks,
            grid,
            title: None,
        }
    }
}

impl Glyph {
    /// The swatch's width and height.
    fn extent(&self) -> [f32; 2] {
        match self {
            Glyph::Fill { .. } => [plan::SWATCH, plan::SWATCH],
            Glyph::Symbol { size, .. } => {
                let side = size.max(0.0).sqrt().max(plan::SWATCH);
                [side, side]
            }
            Glyph::Stroke { width, .. } => [2.0 * plan::SWATCH, width.max(plan::SWATCH)],
        }
    }
}

/// How far apart evenly spaced positions are.
fn spacing(at: &[f32]) -> Option<f32> {
    match at {
        [a, b, ..] => Some((b - a).abs()),
        _ => None,
    }
}

pub(super) fn widest(labels: &[Label]) -> f32 {
    labels.iter().map(|l| l.width).fold(0.0, f32::max)
}
