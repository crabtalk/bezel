//! The legends, as Vega-Lite makes them and Vega draws them: one for each
//! colour, size, shape, dash, stroke width and opacity channel with a
//! scale, those over one field merged into one, each a symbol legend of
//! entries, a gradient of a continuous colour scale, or the steps of a
//! discretizing one; then where each stands beside the plot.

use crate::{
    model,
    model::{
        Align, Anchor, Baseline, Channel, Chart, Datum, Encode, Format, GridAlign, Kind, Layer,
        LegendKind, LegendOrient, Mark, Orient, Overlap, ScaleType, Shape, Side, TickCount,
    },
    plan::{
        Fill, Frame, Glyph, Gradient, Ink, Label, Legend, Paint, Rect, Stroke, Swatch, axis,
        domain::{Colors, Discrete, LayerColor},
        layout,
        layout::Layout,
        look,
        look::{Amount, Dashes, Shapes},
        scales,
        scales::{ColorScale, Colored, Role as ScaleRole, Steps},
    },
    scale,
    scale::{Count, TickFormat},
    time,
};
use d3::scale::{Continuous, Transform};
use gpui::SharedString;

/// Vega's legend defaults, in pixels.
const SYMBOL_SIZE: f32 = 100.0;
const SYMBOL_STROKE_WIDTH: f32 = 1.5;
const SYMBOL_LIMIT: usize = 30;
const LABEL_OFFSET: f32 = 4.0;
const LABEL_LIMIT: f32 = 160.0;
const COLUMN_PADDING: f32 = 10.0;
const ROW_PADDING: f32 = 2.0;
const TITLE_PADDING: f32 = 5.0;
const TITLE_LIMIT: f32 = 180.0;
const GRADIENT_THICKNESS: f32 = 16.0;
const GRADIENT_LABEL_OFFSET: f32 = 2.0;
const GRADIENT_STROKE: Ink = Ink::Faint;
/// Vega-Lite's gradient lengths along the plot's side, least and most, and
/// across a corner.
const VERTICAL_LENGTH: [f32; 2] = [64.0, 200.0];
const HORIZONTAL_LENGTH: [f32; 2] = [100.0, 200.0];
/// Vega's room between legends and what they stand beside, and between
/// legends on one side.
const OFFSET: f32 = 18.0;
const MARGIN: f32 = 8.0;
/// Vega's tick count for a symbol legend of a continuous scale.
const SYMBOL_TICKS: f64 = 5.0;
/// How many places a gradient's colours are sampled at.
const STOPS: usize = 64;

/// A channel a legend can show.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Role {
    Color,
    Fill,
    Stroke,
    Size,
    Shape,
    Dash,
    Width,
    Opacity,
}

impl Role {
    /// Vega-Lite's order: colour, then the rest.
    const ALL: [Role; 8] = [
        Role::Color,
        Role::Size,
        Role::Shape,
        Role::Fill,
        Role::Stroke,
        Role::Dash,
        Role::Width,
        Role::Opacity,
    ];

    fn channel(self, layer: &Layer) -> Option<&Channel> {
        let e = &layer.encoding;
        match self {
            Role::Color => e.color.as_ref()?.channel(),
            Role::Fill => e.fill.as_ref()?.channel(),
            Role::Stroke => e.stroke.as_ref()?.channel(),
            Role::Size => e.size.as_ref()?.channel(),
            Role::Shape => e.shape.as_ref()?.channel(),
            Role::Dash => e.stroke_dash.as_ref()?.channel(),
            Role::Width => e.stroke_width.as_ref()?.channel(),
            Role::Opacity => e.opacity.as_ref()?.channel(),
        }
    }

    fn scale_role(self) -> ScaleRole {
        match self {
            Role::Color | Role::Fill | Role::Stroke => ScaleRole::Color,
            Role::Size => ScaleRole::Size,
            Role::Shape | Role::Dash => ScaleRole::Symbol,
            Role::Width | Role::Opacity => ScaleRole::Amount,
        }
    }

    fn colors(self) -> bool {
        matches!(self, Role::Color | Role::Fill | Role::Stroke)
    }

    /// Where Vega's legend takes its values from first: size, shape, a
    /// colour, stroke width, dash, opacity.
    fn lead(self) -> usize {
        match self {
            Role::Size => 0,
            Role::Shape => 1,
            Role::Color | Role::Fill | Role::Stroke => 2,
            Role::Width => 3,
            Role::Dash => 4,
            Role::Opacity => 5,
        }
    }
}

/// A channel in a legend, the layer it is read from and that layer's place.
struct Member<'a> {
    role: Role,
    layer: &'a Layer,
    index: usize,
    channel: &'a Channel,
    kind: ScaleType,
}

impl Member<'_> {
    /// Whether its scale's domain is discrete: Vega-Lite merges legends
    /// only of the same.
    fn discrete(&self) -> bool {
        scales::listed(self.kind, self.channel) || self.kind == ScaleType::BinOrdinal
    }

    /// Whether its values step through a continuous domain.
    fn stepped(&self) -> bool {
        self.kind.is_discretizing() && !scales::listed(self.kind, self.channel)
    }
}

/// The values a symbol legend lists.
enum Values {
    /// A discrete domain's values, by name.
    Named(Vec<SharedString>),
    Numbers(Vec<f64>),
}

impl Values {
    fn len(&self) -> usize {
        match self {
            Values::Named(names) => names.len(),
            Values::Numbers(numbers) => numbers.len(),
        }
    }
}

/// A legend laid out with its top-left at the origin, and where it asks
/// to stand.
pub(super) struct Block {
    pub(super) legend: Legend,
    pub(super) size: [f32; 2],
    pub(super) orient: LegendOrient,
    pub(super) offset: Option<f32>,
    pub(super) at: [f32; 2],
}

/// The chart's legends, `plot` the plot's width and height that a gradient
/// takes its length from. `None` for a channel over a field its layer
/// lacks.
pub(super) fn legends(
    chart: &Chart,
    colors: &Colors,
    layout: &mut Layout,
    plot: [f32; 2],
) -> Option<Vec<Block>> {
    let mut groups: Vec<Vec<Member>> = Vec::new();
    for role in Role::ALL {
        let Some((index, layer, channel)) = chart
            .layers
            .iter()
            .enumerate()
            .find_map(|(index, layer)| Some((index, layer, role.channel(layer)?)))
        else {
            continue;
        };
        if channel.scale.kind == Some(ScaleType::Identity) || channel.legend.is_none() {
            continue;
        }
        let kind = scales::scale_type(channel, role.scale_role(), layer.mark, false);
        let member = Member {
            role,
            layer,
            index,
            channel,
            kind,
        };
        let key = |m: &Member| m.channel.field().cloned();
        let orient = |m: &Member| m.channel.legend.as_ref().and_then(|l| l.orient);
        let joined = groups.iter_mut().find(|group| {
            let first = &group[0];
            key(first).is_some()
                && key(first) == key(&member)
                && first.discrete() == member.discrete()
                && (orient(first).is_none()
                    || orient(&member).is_none()
                    || orient(first) == orient(&member))
        });
        match joined {
            Some(group) => group.push(member),
            None => groups.push(vec![member]),
        }
    }
    groups
        .into_iter()
        .map(|group| legend(&group, colors, layout, plot))
        .collect::<Option<Vec<_>>>()
        .map(|blocks| blocks.into_iter().flatten().collect())
}

/// The legend properties a group takes: its first member's, then each
/// other's where those leave one unset.
fn merged(group: &[Member]) -> model::Legend {
    let mut spec = group[0]
        .channel
        .legend
        .as_deref()
        .cloned()
        .unwrap_or_default();
    for member in &group[1..] {
        let Some(other) = member.channel.legend.as_deref() else {
            continue;
        };
        spec.orient = spec.orient.or(other.orient);
        spec.direction = spec.direction.or(other.direction);
        spec.format = spec.format.clone().or(other.format.clone());
        spec.values = spec.values.clone().or(other.values.clone());
    }
    spec
}

fn legend(
    group: &[Member],
    colors: &Colors,
    layout: &mut Layout,
    plot: [f32; 2],
) -> Option<Option<Block>> {
    let spec = merged(group);
    let first = &group[0];
    let title = first.channel.title.clone();
    let orient = spec.orient.unwrap_or(LegendOrient::Right);
    let lead = group
        .iter()
        .min_by_key(|member| member.role.lead())
        .expect("a group has a member");
    let format = spec.format.clone().or_else(|| {
        (lead.channel.kind == Kind::Temporal).then(|| Format::Time(time::FORMAT.into()))
    });
    let single_color = group.len() == 1 && first.role.colors();
    let gradient = single_color
        && !first.discrete()
        && spec.kind != Some(LegendKind::Symbol)
        && !first.stepped();
    let discrete_gradient = single_color && first.stepped();
    let gradient_kind = gradient || discrete_gradient;
    let direction = spec.direction.unwrap_or(match orient {
        LegendOrient::Top | LegendOrient::Bottom => Orient::Horizontal,
        LegendOrient::Left | LegendOrient::Right | LegendOrient::None => Orient::Vertical,
        _ if gradient_kind => Orient::Horizontal,
        _ => Orient::Vertical,
    });
    let vertical = direction == Orient::Vertical;
    let mut out = Legend {
        frame: None,
        title: None,
        swatches: Vec::new(),
        gradient: None,
        labels: Vec::new(),
        over: spec.zindex.is_some_and(|z| z > 0.0),
    };
    let label_offset = spec.label_offset;
    let overlap = spec.label_overlap.unwrap_or(match lead.kind {
        ScaleType::Quantile | ScaleType::Threshold | ScaleType::Log | ScaleType::Symlog => {
            Overlap::Greedy
        }
        _ => Overlap::Parity,
    });
    if gradient_kind {
        let length = spec.ramp.length.unwrap_or(match (vertical, orient) {
            (true, _) => plot[1].clamp(VERTICAL_LENGTH[0], VERTICAL_LENGTH[1]),
            (false, LegendOrient::Top | LegendOrient::Bottom) => {
                plot[0].clamp(HORIZONTAL_LENGTH[0], HORIZONTAL_LENGTH[1])
            }
            (false, _) => HORIZONTAL_LENGTH[0],
        });
        let thickness = spec.ramp.thickness.unwrap_or(GRADIENT_THICKNESS);
        let palette = palette(first, colors)?;
        let rect = match vertical {
            true => Rect {
                x: 0.0,
                y: 0.0,
                w: thickness,
                h: length,
            },
            false => Rect {
                x: 0.0,
                y: 0.0,
                w: length,
                h: thickness,
            },
        };
        // A place along the run, from 0 at its start, on screen.
        let along = |fraction: f32| match vertical {
            true => (1.0 - fraction) * length,
            false => fraction * length,
        };
        let (fill, places) = match gradient {
            true => {
                let scale = palette.continuous()?;
                let count = match spec.tick_count {
                    Some(count) => count,
                    None => TickCount::Count(f64::from((2.0 * (length / 100.0).floor()).max(2.0))),
                };
                let count = Count::of(&scale, count, spec.tick_min_step, 0)?;
                let given = numbers(spec.values.as_deref());
                let mut values = given
                    .clone()
                    .unwrap_or_else(|| scale::values(&scale, count, None));
                let (first, last) = (scale.domain[0], scale.domain[scale.domain.len() - 1]);
                if values.len() < 3 && given.is_none() && first != last {
                    values = vec![first, last];
                }
                let tick = TickFormat::of(
                    &scale,
                    count.number(),
                    format.as_ref(),
                    values.len(),
                    given.is_some(),
                );
                let unit =
                    Continuous::new(scale.transform.clone(), vec![first, last], vec![0.0, 1.0]);
                let stops = (0..=STOPS)
                    .filter_map(|i| {
                        let t = i as f64 / STOPS as f64;
                        Some((t as f32, palette.value(unit.invert(t))?))
                    })
                    .collect();
                let places: Vec<(f32, String, Datum)> = values
                    .iter()
                    .map(|&v| (unit.at(v) as f32, tick.label(v), Datum::Number(v)))
                    .collect();
                (Fill::Stops(stops), places)
            }
            false => {
                let ColorScale::Steps { steps, inks } = &palette else {
                    return None;
                };
                let (values, _) = steps.values();
                let tick = steps.label_format(format.as_ref());
                let labels: Vec<Option<String>> = values
                    .iter()
                    .enumerate()
                    .map(|(i, &v)| (i > 0).then(|| tick.label(v)))
                    .collect();
                let (lo, hi) = steps.ends();
                let fraction = |v: f64| ((v - lo) / (hi - lo)) as f32;
                let n = values.len();
                let cells = (0..n)
                    .filter_map(|i| {
                        let start = if i == 0 { 0.0 } else { fraction(values[i]) };
                        let end = if i + 1 == n {
                            1.0
                        } else {
                            fraction(values[i + 1])
                        };
                        let ink = palette.value(values[i]).or_else(|| inks.get(i).copied())?;
                        let (a, b) = (along(start), along(end));
                        let cell = match vertical {
                            true => Rect {
                                x: 0.0,
                                y: a.min(b),
                                w: thickness,
                                h: (a - b).abs(),
                            },
                            false => Rect {
                                x: a.min(b),
                                y: 0.0,
                                w: (a - b).abs(),
                                h: thickness,
                            },
                        };
                        Some((cell, ink))
                    })
                    .collect();
                let places = values
                    .iter()
                    .zip(labels)
                    .enumerate()
                    .filter_map(|(i, (&v, label))| {
                        let place = if i == 0 { 0.0 } else { fraction(v) };
                        Some((place, label?, Datum::Number(v)))
                    })
                    .collect();
                (Fill::Bands(cells), places)
            }
        };
        let stroke_width = spec.ramp.stroke_width.unwrap_or(0.0);
        out.gradient = Some(Gradient {
            rect,
            vertical,
            fill,
            stroke: (stroke_width > 0.0).then(|| Stroke {
                paint: Paint {
                    ink: spec.ramp.stroke.map_or(GRADIENT_STROKE, Ink::Fixed),
                    alpha: 1.0,
                },
                width: stroke_width,
                dash: None,
                dash_offset: 0.0,
                cap: model::Cap::Butt,
            }),
            opacity: spec.ramp.opacity.unwrap_or(1.0),
        });
        let offset = label_offset.unwrap_or(GRADIENT_LABEL_OFFSET);
        let last = places.len().saturating_sub(1).max(1) as f64;
        let mut labels: Vec<Option<Label>> = places
            .into_iter()
            .enumerate()
            .map(|(i, (place, text, value))| {
                let text = relabelled(&spec, &value, text, i as f64 / last);
                let label = layout.label(&text, &spec.label, (1.0, false), Ink::Muted, 0.0);
                let at = along(place);
                Some(match vertical {
                    true => label.anchored(
                        [thickness + offset, at],
                        spec.label.align.unwrap_or(Align::Left),
                        spec.label.baseline.unwrap_or(match place {
                            p if p <= 0.0 => Baseline::Bottom,
                            p if p >= 1.0 => Baseline::Top,
                            _ => Baseline::Middle,
                        }),
                    ),
                    false => label.anchored(
                        [at, thickness + offset],
                        spec.label.align.unwrap_or(match place {
                            p if p <= 0.0 => Align::Left,
                            p if p >= 1.0 => Align::Right,
                            _ => Align::Center,
                        }),
                        spec.label.baseline.unwrap_or(Baseline::Top),
                    ),
                })
            })
            .collect();
        axis::overlap(&mut labels, overlap, spec.label_separation.unwrap_or(0.0));
        out.labels = labels.into_iter().flatten().collect();
    } else {
        let (values, labels) = symbol_values(group, lead, colors, &spec, format.as_ref())?;
        let limit = spec.symbols.limit.unwrap_or(SYMBOL_LIMIT);
        let mut entries: Vec<(Glyph, String, Datum)> = Vec::new();
        let count = values.len();
        let shown = match limit > 0 && count > limit {
            true => limit - 1,
            false => count,
        };
        for i in 0..shown {
            let glyph = glyph(group, &values, i, colors, &spec)?;
            let value = match &values {
                Values::Named(names) => Datum::Text(names[i].clone()),
                Values::Numbers(numbers) => Datum::Number(numbers[i]),
            };
            entries.push((glyph, labels[i].clone(), value));
        }
        if shown < count {
            let glyph = glyph(group, &values, shown, colors, &spec)?;
            entries.push((
                glyph,
                format!("\u{2026}{} entries", count - shown),
                Datum::Number(f64::NAN),
            ));
        }
        // A continuous size scale's first value at no size is left out.
        if lead.role == Role::Size
            && spec.values.is_none()
            && matches!(values, Values::Numbers(_))
            && entries.first().is_some_and(|(glyph, ..)| glyph.size <= 0.0)
        {
            entries.remove(0);
        }
        if entries.is_empty() {
            return Some(None);
        }
        let font_size = layout.face(&spec.label, 1.0, false).size;
        let last = entries.len().saturating_sub(1).max(1) as f64;
        let measured: Vec<(Glyph, f32, Label)> = entries
            .into_iter()
            .enumerate()
            .map(|(i, (glyph, text, value))| {
                let text = relabelled(&spec, &value, text, i as f64 / last);
                // Vega's entry size: the symbol's width and its stroke, at
                // least the label's font size.
                let size = (glyph.size.max(0.0).sqrt() + glyph.stroke_width)
                    .ceil()
                    .max(font_size);
                let label = layout.label(&text, &spec.label, (1.0, false), Ink::Muted, LABEL_LIMIT);
                (glyph, size, label)
            })
            .collect();
        let widest = measured
            .iter()
            .map(|(_, size, _)| *size)
            .fold(0.0, f32::max);
        let columns = match spec.columns {
            Some(columns) => columns,
            None if vertical => 1,
            None => 0,
        };
        let symbol_offset = spec.symbols.offset.unwrap_or(0.0);
        let label_offset = label_offset.unwrap_or(LABEL_OFFSET);
        let clip = spec.clip_height;
        // Each entry's box at the origin: its swatch and its label.
        let boxes: Vec<(Swatch, [f32; 2])> = measured
            .into_iter()
            .map(|(glyph, size, label)| {
                let x = if columns > 0 { widest } else { size };
                let height = clip.unwrap_or(size);
                let center = [x / 2.0 + symbol_offset, height / 2.0];
                let label = label.anchored(
                    [x + symbol_offset + label_offset, center[1]],
                    spec.label.align.unwrap_or(Align::Left),
                    spec.label.baseline.unwrap_or(Baseline::Middle),
                );
                let swatch = Rect {
                    x: center[0] - size / 2.0,
                    y: center[1] - size / 2.0,
                    w: size,
                    h: size,
                };
                let r = label.rect().union(swatch);
                let clip = clip.map(|h| Rect {
                    x: swatch.x,
                    y: 0.0,
                    w: swatch.w,
                    h,
                });
                let bounds = [r.right(), r.bottom().max(height)];
                (
                    Swatch {
                        glyph,
                        swatch,
                        label,
                        clip,
                    },
                    bounds,
                )
            })
            .collect();
        let n = boxes.len();
        let ncols = match columns {
            0 => n,
            c => c.min(n),
        }
        .max(1);
        let nrows = n.div_ceil(ncols);
        let place = |i: usize| -> (usize, usize) {
            match (vertical && columns > 0, columns) {
                (_, 0) => (0, i),
                (true, _) => (i % nrows, i / nrows),
                (false, _) => (i / ncols, i % ncols),
            }
        };
        let row_padding = spec.row_padding.unwrap_or(ROW_PADDING);
        let column_padding = spec.column_padding.unwrap_or(COLUMN_PADDING);
        let cols = (0..n).map(|i| place(i).1).max().map_or(0, |c| c + 1);
        let rows = (0..n).map(|i| place(i).0).max().map_or(0, |r| r + 1);
        let mut widths = vec![0.0f32; cols];
        let mut heights = vec![0.0f32; rows];
        for (i, (_, [w, h])) in boxes.iter().enumerate() {
            let (r, c) = place(i);
            widths[c] = widths[c].max(*w);
            heights[r] = heights[r].max(*h);
        }
        let align = spec.grid_align.unwrap_or(GridAlign::Each);
        if align == GridAlign::All {
            let widest = widths.iter().copied().fold(0.0, f32::max);
            widths.iter_mut().for_each(|w| *w = widest);
        }
        let xs: Vec<f32> = widths
            .iter()
            .scan(0.0, |x, w| {
                let at = *x;
                *x += w + column_padding;
                Some(at)
            })
            .collect();
        let ys: Vec<f32> = heights
            .iter()
            .scan(0.0, |y, h| {
                let at = *y;
                *y += h + row_padding;
                Some(at)
            })
            .collect();
        // Without alignment, entries in a row stand one after another.
        let mut flowing = vec![0.0f32; rows];
        for (i, (swatch, [w, h])) in boxes.into_iter().enumerate() {
            let (r, c) = place(i);
            let x = match align {
                GridAlign::None => {
                    let x = flowing[r];
                    flowing[r] += w + column_padding;
                    x
                }
                _ => xs[c],
            };
            let by = [x, ys[r] + (heights[r] - h) / 2.0];
            out.swatches.push(Swatch {
                swatch: swatch.swatch.moved(by),
                label: swatch.label.moved(by),
                clip: swatch.clip.map(|clip| clip.moved(by)),
                glyph: swatch.glyph,
            });
        }
    }

    // The title, on its side of the entries, and the box round them.
    let padding = spec.padding.unwrap_or(0.0);
    let entries = out.bounds();
    let title = title.filter(|t| !t.is_empty()).map(|text| {
        layout.label(
            &text,
            &spec.title,
            (layout::GUIDE_TITLE, true),
            Ink::Text,
            TITLE_LIMIT,
        )
    });
    let title_padding = spec.title_padding.unwrap_or(TITLE_PADDING);
    let title_orient = spec.title_orient.unwrap_or(Side::Top);
    let (entries_at, title) = match (title, entries) {
        (Some(title), Some(entries)) => {
            let anchor = spec.title_anchor.unwrap_or(match title_orient {
                Side::Left | Side::Right => Anchor::Middle,
                _ => Anchor::Start,
            });
            let (w, h) = (title.width, title.height());
            let fraction = anchor.fraction();
            match title_orient {
                Side::Top => {
                    let x = (entries.w - w) * fraction;
                    (
                        [0.0, h + title_padding],
                        Some(title.moved([x.max(0.0), 0.0])),
                    )
                }
                Side::Bottom => {
                    let x = (entries.w - w) * fraction;
                    (
                        [0.0, 0.0],
                        Some(title.moved([x.max(0.0), entries.h + title_padding])),
                    )
                }
                Side::Left => {
                    let y = (entries.h - h) * fraction;
                    (
                        [w + title_padding, 0.0],
                        Some(title.moved([0.0, y.max(0.0)])),
                    )
                }
                Side::Right => {
                    let y = (entries.h - h) * fraction;
                    (
                        [0.0, 0.0],
                        Some(title.moved([entries.w + title_padding, y.max(0.0)])),
                    )
                }
            }
        }
        (_, None) => return Some(None),
        (None, Some(_)) => ([0.0, 0.0], None),
    };
    let entries_origin = entries.map_or([0.0, 0.0], |r| [r.x, r.y]);
    out = out.moved([
        entries_at[0] - entries_origin[0] + padding,
        entries_at[1] - entries_origin[1] + padding,
    ]);
    out.title = title.map(|title| title.moved([padding, padding]));
    let content = out.bounds().unwrap_or_default();
    let size = [
        (content.right() + padding).ceil(),
        (content.bottom() + padding).ceil(),
    ];
    if spec.fill.is_some() || spec.stroke.is_some() {
        out.frame = Some(Frame {
            rect: Rect {
                x: 0.0,
                y: 0.0,
                w: size[0],
                h: size[1],
            },
            fill: spec.fill.map(|c| Paint {
                ink: Ink::Fixed(c),
                alpha: 1.0,
            }),
            stroke: spec.stroke.map(|c| Paint {
                ink: Ink::Fixed(c),
                alpha: 1.0,
            }),
            radius: spec.corner_radius.unwrap_or(0.0),
        });
    }
    Some(Some(Block {
        legend: out,
        size,
        orient,
        offset: spec.offset,
        at: [spec.legend_x.unwrap_or(0.0), spec.legend_y.unwrap_or(0.0)],
    }))
}

/// `text` as `spec`'s `labelExpr` rewrites it, at `index` of 0 to 1.
fn relabelled(spec: &model::Legend, value: &Datum, text: String, index: f64) -> String {
    match &spec.relabel {
        Some(relabel) => relabel.0(value, &text, index).to_string(),
        None => text,
    }
}

/// `values`' numbers, `None` without them.
fn numbers(values: Option<&[Datum]>) -> Option<Vec<f64>> {
    Some(
        values?
            .iter()
            .filter_map(|value| match value {
                Datum::Number(number) => Some(*number),
                Datum::Text(text) => text.parse().ok(),
            })
            .collect(),
    )
}

/// A colour member's scale: the chart's colour scale, or a fill's or
/// stroke's own.
fn palette(member: &Member, colors: &Colors) -> Option<ColorScale> {
    match member.role {
        Role::Color => colors.palette.clone(),
        _ => Some(ColorScale::over(member.channel, member.layer)?.0),
    }
}

/// The values a symbol legend lists, from `lead`'s scale, and their labels.
fn symbol_values(
    group: &[Member],
    lead: &Member,
    colors: &Colors,
    spec: &model::Legend,
    format: Option<&Format>,
) -> Option<(Values, Vec<String>)> {
    if lead.discrete() && !lead.stepped() {
        let names: Vec<SharedString> = match &spec.values {
            Some(values) => values.iter().map(axis::named).collect(),
            None => {
                let mut names: Vec<SharedString> = Vec::new();
                for member in group.iter().filter(|m| m.discrete()) {
                    for name in domain_of(member, Some(colors))? {
                        if !names.contains(&name) {
                            names.push(name);
                        }
                    }
                }
                names
            }
        };
        let labels = names
            .iter()
            .map(|name| axis::discrete(name, Kind::Nominal, format).1)
            .collect();
        return Some((Values::Named(names), labels));
    }
    // A continuous, binned or discretizing scale.
    let steps = match lead.role {
        Role::Color => match &colors.palette {
            Some(ColorScale::Steps { steps, .. }) => Some(steps.clone()),
            _ => None,
        },
        Role::Fill | Role::Stroke => match ColorScale::over(lead.channel, lead.layer)?.0 {
            ColorScale::Steps { steps, .. } => Some(steps),
            _ => None,
        },
        _ => match amount(lead)? {
            Amount::Steps { steps, .. } => Some(steps),
            _ => None,
        },
    };
    if let Some(steps) = steps {
        let (values, max) = steps.values();
        let tick = steps.label_format(format);
        let labels = ranges(&values, max, |v| tick.label(v));
        return Some((Values::Numbers(values), labels));
    }
    let scale: Continuous = match lead.role {
        Role::Color | Role::Fill | Role::Stroke => palette(lead, colors)?.continuous()?,
        _ => match amount(lead)? {
            Amount::Continuous { scale, .. } => scale,
            _ => return None,
        },
    };
    let edges = scales::edges(&lead.channel.scale);
    let count = spec.tick_count.unwrap_or(TickCount::Count(SYMBOL_TICKS));
    let count = Count::of(
        &scale,
        count,
        spec.tick_min_step,
        edges.as_ref().map_or(0, Vec::len),
    )?;
    let given = numbers(spec.values.as_deref());
    if let (Some(edges), None) = (&edges, &given) {
        // Each bin from its start to the next's.
        let starts = edges[..edges.len() - 1].to_vec();
        let max = edges[edges.len() - 1];
        let first = edges[0];
        let span = Continuous::new(Transform::Linear, vec![first, max], vec![0.0, 1.0]);
        let tick = TickFormat::of(&span, count.number(), format, starts.len(), true);
        let labels = ranges(&starts, max, |v| tick.label(v));
        return Some((Values::Numbers(starts), labels));
    }
    let values = match &given {
        Some(values) => values.clone(),
        None => scale::values(&scale, count, None),
    };
    let tick = TickFormat::of(&scale, count.number(), format, values.len(), true);
    let labels: Vec<String> = values.iter().map(|&v| tick.label(v)).collect();
    Some((Values::Numbers(values), labels))
}

/// Vega's `formatRange`: each of `starts` to the next, the last to `max`;
/// an end that is not finite reads `< hi` or `≥ lo`.
fn ranges(starts: &[f64], max: f64, label: impl Fn(f64) -> String) -> Vec<String> {
    (0..starts.len())
        .map(|i| {
            let limit = starts.get(i + 1).copied().unwrap_or(max);
            let lo = starts[i].is_finite().then(|| label(starts[i]));
            let hi = limit.is_finite().then(|| label(limit));
            match (lo, hi) {
                (Some(lo), Some(hi)) => format!("{lo} \u{2013} {hi}"),
                (None, Some(hi)) => format!("< {hi}"),
                (lo, None) => format!("\u{2265} {}", lo.unwrap_or_default()),
            }
        })
        .collect()
}

/// A discrete member's domain, by name: the chart's colours' for the
/// colour channel where `colors` are given.
fn domain_of(member: &Member, colors: Option<&Colors>) -> Option<Vec<SharedString>> {
    if let (Role::Color, Some(colors)) = (member.role, colors) {
        return Some(
            colors
                .colors
                .iter()
                .filter_map(|c| c.name.clone())
                .collect(),
        );
    }
    let column = member.channel.column(&member.layer.data)?;
    Some(
        Discrete::of(&column, &member.channel.sort)
            .within(&member.channel.scale)
            .labels,
    )
}

/// A size, stroke width or opacity member's scale onto its range.
fn amount(member: &Member) -> Option<Amount> {
    let layer = member.layer;
    let (range, size) = match member.role {
        Role::Size if layer.mark.is_symbol() => (look::POINT_SIZE, true),
        Role::Size => (look::STROKE_WIDTH, true),
        Role::Width => (look::STROKE_WIDTH, false),
        Role::Opacity => (look::OPACITY, false),
        _ => return None,
    };
    Amount::of(member.channel, &layer.data, range, layer.mark, size)
}

/// Entry `i`'s symbol: Vega-Lite's for the group's mark, through each
/// member's scale.
fn glyph(
    group: &[Member],
    values: &Values,
    i: usize,
    colors: &Colors,
    spec: &model::Legend,
) -> Option<Glyph> {
    let first = &group[0];
    let layer = first.layer;
    let mark = layer.mark;
    let filled = layer.properties.filled.unwrap_or(mark.filled()) && mark != Mark::Trail;
    let fixed_shape = match &layer.encoding.shape {
        Some(Encode::Value(shape)) => Some(shape.clone()),
        _ => None,
    };
    let mut glyph = Glyph {
        shape: spec
            .symbols
            .shape
            .clone()
            .or(fixed_shape)
            .unwrap_or(match mark {
                Mark::Bar | Mark::Rect | Mark::Image | Mark::Square => Shape::Square,
                Mark::Line | Mark::Trail | Mark::Rule => Shape::Stroke,
                _ => Shape::Circle,
            }),
        size: spec.symbols.size.unwrap_or(SYMBOL_SIZE),
        fill: None,
        stroke: None,
        stroke_width: spec.symbols.stroke_width.unwrap_or(match mark {
            Mark::Line | Mark::Trail | Mark::Rule => look::LINE_WIDTH,
            _ => SYMBOL_STROKE_WIDTH,
        }),
        dash: spec.symbols.dash.as_deref().and_then(look::solid),
        dash_offset: spec.symbols.dash_offset.unwrap_or(0.0),
    };
    // A colour the group does not set: the layer's own, or black where a
    // field colours it.
    let base = match &colors.layers[first.index] {
        LayerColor::Fixed(color) => colors.colors[*color].ink,
        LayerColor::Field(_) => Ink::Text,
    };
    let mut fill = filled.then_some(base);
    let mut stroke = (!filled).then_some(base);
    let value = |member: &Member| -> Option<MemberValue> {
        Some(match values {
            Values::Named(names) => MemberValue::Named(names[i].clone(), member.discrete()),
            Values::Numbers(numbers) => MemberValue::Number(numbers[i]),
        })
    };
    for member in group {
        let at = value(member)?;
        match member.role {
            Role::Color | Role::Fill | Role::Stroke => {
                let ink = ink_of(member, colors, &at)?;
                let paints_fill = match member.role {
                    Role::Fill => true,
                    Role::Stroke => false,
                    _ => filled,
                };
                match paints_fill {
                    true => {
                        fill = ink;
                        if member.role == Role::Color {
                            stroke = None;
                        }
                    }
                    false => {
                        stroke = ink;
                        if member.role == Role::Color {
                            fill = None;
                        }
                    }
                }
            }
            Role::Shape => {
                let shapes = Shapes::of(member.channel)?;
                glyph.shape = match place_of(member, &at, shapes.len())? {
                    Some(place) => shapes.nth(place),
                    None => glyph.shape,
                };
            }
            Role::Dash => {
                let dashes = Dashes::of(member.channel)?;
                if let Some(place) = place_of(member, &at, dashes.len())? {
                    glyph.dash = dashes.nth(place);
                }
            }
            Role::Size | Role::Width | Role::Opacity => {
                let Some(v) = amount(member)?.of_value(&at.datum()) else {
                    continue;
                };
                match (member.role, member.layer.mark.is_symbol()) {
                    (Role::Size, true) => glyph.size = v,
                    (Role::Size, false) | (Role::Width, _) => glyph.stroke_width = v,
                    _ => {}
                }
            }
        }
    }
    let opacity = spec
        .symbols
        .opacity
        .or_else(|| {
            let member = group.iter().find(|m| m.role == Role::Opacity)?;
            let at = value(member)?;
            amount(member)?.of_value(&at.datum())
        })
        .or(match &layer.encoding.opacity {
            Some(Encode::Value(opacity)) => Some(*opacity),
            _ => None,
        })
        .unwrap_or(match mark {
            Mark::Point | Mark::Circle | Mark::Square | Mark::Tick => look::REDUCED_OPACITY,
            _ => 1.0,
        });
    let paint = |ink: Ink| Paint {
        ink,
        alpha: opacity,
    };
    glyph.fill = spec.symbols.fill.map(Ink::Fixed).or(fill).map(paint);
    glyph.stroke = spec.symbols.stroke.map(Ink::Fixed).or(stroke).map(paint);
    if glyph.shape == Shape::Stroke && glyph.stroke.is_none() {
        glyph.stroke = glyph.fill.take();
    }
    Some(glyph)
}

/// A legend value as one member reads it.
enum MemberValue {
    /// A name, and whether the member's domain is discrete.
    Named(SharedString, bool),
    Number(f64),
}

impl MemberValue {
    fn datum(&self) -> Datum {
        match self {
            MemberValue::Named(name, _) => match name.parse::<f64>() {
                Ok(number) => Datum::Number(number),
                Err(_) => Datum::Text(name.clone()),
            },
            MemberValue::Number(number) => Datum::Number(*number),
        }
    }
}

/// A shape or dash member's place in its range of `count` for `at`; `None`
/// inside for a value its domain lacks.
fn place_of(member: &Member, at: &MemberValue, count: usize) -> Option<Option<usize>> {
    if member.kind.is_discretizing() {
        let column = member.channel.column(&member.layer.data)?;
        let values = column.numbers()?;
        let steps = Steps::of(member.kind, &member.channel.scale, &values, count)?;
        let number = match at.datum() {
            Datum::Number(number) => number,
            Datum::Text(_) => return Some(None),
        };
        return Some(steps.index(number));
    }
    let names = domain_of(member, None)?;
    let name = match at {
        MemberValue::Named(name, _) => name.clone(),
        MemberValue::Number(number) => axis::named(&Datum::Number(*number)),
    };
    Some(names.iter().position(|n| *n == name))
}

/// A colour member's ink for `at`.
fn ink_of(member: &Member, colors: &Colors, at: &MemberValue) -> Option<Option<Ink>> {
    let name = match at {
        MemberValue::Named(name, true) => Some(name.clone()),
        _ => None,
    };
    if member.role == Role::Color {
        return Some(match (name, &colors.palette) {
            (Some(name), _) => colors
                .colors
                .iter()
                .find(|c| c.name.as_ref() == Some(&name))
                .map(|c| c.ink),
            (None, Some(palette)) => match at.datum() {
                Datum::Number(v) => palette.value(v),
                Datum::Text(_) => None,
            },
            (None, None) => None,
        });
    }
    if let Some(rows) = scales::named_inks(member.channel, member.layer)? {
        let column = member.channel.column(&member.layer.data)?;
        let domain = Discrete::of(&column, &member.channel.sort);
        let name = name?;
        let code = domain.labels.iter().position(|l| *l == name)?;
        let row = domain.codes.iter().position(|&c| c as usize == code)?;
        return Some(rows[row]);
    }
    Some(match ColorScale::over(member.channel, member.layer)? {
        (palette, Colored::Discrete(domain)) => {
            let name = name?;
            let code = domain.labels.iter().position(|l| *l == name)?;
            palette.discrete(code)
        }
        (palette, Colored::Numbers(_)) => match at.datum() {
            Datum::Number(v) => palette.value(v),
            Datum::Text(_) => None,
        },
    })
}

/// What `blocks` take from the inner room, each side's legends side by
/// side or one under another.
pub(super) fn reserve(blocks: &[Block], layout: &mut Layout) {
    for side in [Side::Top, Side::Bottom, Side::Left, Side::Right] {
        let orient = orient_of(side);
        let group: Vec<&Block> = blocks.iter().filter(|b| b.orient == orient).collect();
        if group.is_empty() {
            continue;
        }
        let offset = group
            .iter()
            .filter_map(|b| b.offset)
            .reduce(f32::max)
            .unwrap_or(OFFSET);
        let extent = match side.is_horizontal() {
            true => group.iter().map(|b| b.size[1]).fold(0.0, f32::max),
            false => group.iter().map(|b| b.size[0]).fold(0.0, f32::max),
        };
        layout.reserve(side, extent + offset);
    }
}

fn orient_of(side: Side) -> LegendOrient {
    match side {
        Side::Top => LegendOrient::Top,
        Side::Bottom => LegendOrient::Bottom,
        Side::Left => LegendOrient::Left,
        Side::Right => LegendOrient::Right,
    }
}

/// `blocks` placed: each side's beside `bounds`, the plot and its axes,
/// from the plot's start; a corner's inside `plot`; one of orient `None`
/// at its own place from the plot's top-left.
pub(super) fn place(blocks: Vec<Block>, plot: Rect, bounds: Rect) -> Vec<Legend> {
    let offsets = |orient: LegendOrient| {
        blocks
            .iter()
            .filter(|b| b.orient == orient)
            .filter_map(|b| b.offset)
            .reduce(f32::max)
            .unwrap_or(OFFSET)
    };
    let offset_of =
        [Side::Top, Side::Bottom, Side::Left, Side::Right].map(|s| offsets(orient_of(s)));
    let corner = |b: &Block| b.offset.unwrap_or(OFFSET);
    let mut runs = [0.0f32; 4];
    blocks
        .into_iter()
        .map(|block| {
            let [w, h] = block.size;
            let at = match block.orient {
                LegendOrient::Top => {
                    let x = plot.x + runs[0];
                    runs[0] += w + MARGIN;
                    [x, bounds.y - offset_of[0] - h]
                }
                LegendOrient::Bottom => {
                    let x = plot.x + runs[1];
                    runs[1] += w + MARGIN;
                    [x, bounds.bottom() + offset_of[1]]
                }
                LegendOrient::Left => {
                    let y = plot.y + runs[2];
                    runs[2] += h + MARGIN;
                    [bounds.x - offset_of[2] - w, y]
                }
                LegendOrient::Right => {
                    let y = plot.y + runs[3];
                    runs[3] += h + MARGIN;
                    [bounds.right() + offset_of[3], y]
                }
                LegendOrient::TopLeft => [plot.x + corner(&block), plot.y + corner(&block)],
                LegendOrient::TopRight => {
                    [plot.right() - corner(&block) - w, plot.y + corner(&block)]
                }
                LegendOrient::BottomLeft => {
                    [plot.x + corner(&block), plot.bottom() - corner(&block) - h]
                }
                LegendOrient::BottomRight => [
                    plot.right() - corner(&block) - w,
                    plot.bottom() - corner(&block) - h,
                ],
                LegendOrient::None => [plot.x + block.at[0], plot.y + block.at[1]],
            };
            block.legend.moved(at)
        })
        .collect()
}

impl Legend {
    /// What its title, entries, ramp and labels cover.
    pub fn bounds(&self) -> Option<Rect> {
        let swatches = self
            .swatches
            .iter()
            .map(|swatch| swatch.swatch.union(swatch.label.rect()));
        let gradient = self.gradient.iter().map(|gradient| gradient.rect);
        let texts = self.title.iter().chain(&self.labels).map(Label::rect);
        let frame = self.frame.iter().map(|frame| frame.rect);
        swatches
            .chain(gradient)
            .chain(texts)
            .chain(frame)
            .reduce(Rect::union)
    }

    /// Moved by `by`.
    pub fn moved(self, by: [f32; 2]) -> Self {
        Legend {
            frame: self.frame.map(|frame| Frame {
                rect: frame.rect.moved(by),
                ..frame
            }),
            title: self.title.map(|title| title.moved(by)),
            swatches: self
                .swatches
                .into_iter()
                .map(|swatch| Swatch {
                    swatch: swatch.swatch.moved(by),
                    label: swatch.label.moved(by),
                    clip: swatch.clip.map(|clip| clip.moved(by)),
                    glyph: swatch.glyph,
                })
                .collect(),
            gradient: self.gradient.map(|gradient| Gradient {
                rect: gradient.rect.moved(by),
                fill: match gradient.fill {
                    Fill::Bands(bands) => Fill::Bands(
                        bands
                            .into_iter()
                            .map(|(rect, ink)| (rect.moved(by), ink))
                            .collect(),
                    ),
                    stops => stops,
                },
                ..gradient
            }),
            labels: self
                .labels
                .into_iter()
                .map(|label| label.moved(by))
                .collect(),
            over: self.over,
        }
    }
}
