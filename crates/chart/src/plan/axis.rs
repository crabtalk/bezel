//! An x or y axis as Vega-Lite defaults it and Vega draws it: its domain
//! line, ticks, grid lines, labels and title beside the plot.
//!
//! TODO(DEV-205): Vega-Lite turns a discrete x's labels upright, and Vega
//! a y axis's title. Here the labels are drawn level, every other one
//! hidden while they overlap, and the title level over the axis's top end
//! unless its `titleAngle` is written.

use crate::{
    model,
    model::{
        Align, Anchor, Baseline, Cap, Channel, Datum, Format, Kind, Line, Mark, Overlap, Side,
        TickBand, TickCount, TickStyle,
    },
    plan::{Axis, Ink, Label, Paint, Rect, Rule, Stroke, layout, layout::Layout, look},
    scale,
    scale::{Count, TickFormat},
};
use d3::{
    format::number::Number as JsNumber,
    scale::{Band, Continuous, Transform},
    time::date::LocalZone,
};
use gpui::SharedString;

/// Vega's axis defaults, in pixels.
const TICK_SIZE: f32 = 5.0;
const LABEL_PADDING: f32 = 2.0;
const LABEL_LIMIT: f32 = 180.0;
const TITLE_PADDING: f32 = 4.0;
const MAX_EXTENT: f32 = 200.0;
const LINE_WIDTH: f32 = 1.0;
/// Vega-Lite's tick count: one to this many pixels of the axis, and of a
/// binned axis one to `BIN_SPACING`.
const TICK_SPACING: f32 = 40.0;
const BIN_SPACING: f32 = 10.0;
/// Vega's tick count on a log scale.
const LOG_TICKS: f64 = 10.0;

/// What an axis ticks along its run.
pub(super) enum Ticked<'a> {
    /// A band or point scale's bands, named by `labels`.
    Bands(&'a Band, &'a [SharedString]),
    Continuous(&'a Continuous),
}

/// A position channel and its scale on screen.
pub(super) struct Scaled<'a> {
    pub(super) channel: &'a Channel,
    /// The mark of the layer it is read from.
    pub(super) mark: Mark,
    /// x, rather than y.
    pub(super) x: bool,
    pub(super) ticked: Ticked<'a>,
    pub(super) title: Option<SharedString>,
}

/// A tick: its value, where its tick and the middle of its band stand
/// along the run, and its label's text as formatted and as shown.
struct Tick {
    value: Datum,
    at: f32,
    middle: f32,
    formatted: String,
    text: String,
}

/// A value of a discrete scale as its guide writes it: through `format`,
/// or a time on the calendar for a temporal field, where it is a number.
pub(super) fn discrete(
    label: &SharedString,
    kind: Kind,
    format: Option<&Format>,
) -> (Datum, String) {
    match (label.parse::<f64>().ok(), format) {
        (Some(value), Some(format)) => (Datum::Number(value), format.write(value)),
        (Some(value), None) if kind == Kind::Temporal => {
            let zone = LocalZone::utc();
            (Datum::Number(value), zone.date(value).multi_format())
        }
        (Some(value), None) => (Datum::Number(value), label.to_string()),
        (None, _) => (Datum::Text(label.clone()), label.to_string()),
    }
}

/// A datum's text, as a discrete scale names its value.
pub(super) fn named(datum: &Datum) -> SharedString {
    match datum {
        Datum::Number(value) => JsNumber(*value).to_string().into(),
        Datum::Text(text) => text.clone(),
    }
}

/// How `line` strokes, `ink` and Vega's width where it says nothing.
/// `None` where it is not drawn.
pub(super) fn stroke(line: &Line, ink: Ink, width: f32) -> Stroke {
    Stroke {
        paint: Paint {
            ink: line.color.map_or(ink, Ink::Fixed),
            alpha: line.opacity.unwrap_or(1.0),
        },
        width: line.width.unwrap_or(width),
        dash: line.dash.as_deref().and_then(look::solid),
        dash_offset: line.dash_offset.unwrap_or(0.0),
        cap: line.cap.unwrap_or(Cap::Butt),
    }
}

/// Hides labels as Vega's `labelOverlap` does: `labels` in tick order,
/// each `None` once hidden, `sep` the least room between two.
pub(super) fn overlap(labels: &mut [Option<Label>], method: Overlap, sep: f32) {
    let apart = |a: &Rect, b: &Rect| {
        sep > (b.x - a.right())
            .max(a.x - b.right())
            .max(b.y - a.bottom())
            .max(a.y - b.bottom())
    };
    let rects: Vec<Option<Rect>> = labels
        .iter()
        .map(|label| {
            label
                .as_ref()
                .map(Label::rect)
                .filter(|r| r.w > 1.0 && r.h > 1.0)
        })
        .collect();
    let source: Vec<usize> = (0..labels.len()).filter(|&i| rects[i].is_some()).collect();
    let rect = |i: usize| rects[i].expect("kept labels have bounds");
    let overlaps = |items: &[usize]| items.windows(2).any(|w| apart(&rect(w[0]), &rect(w[1])));
    let mut items = source.clone();
    if method == Overlap::Off || items.len() < 3 || !overlaps(&items) {
        return;
    }
    loop {
        items = match method {
            Overlap::Greedy => {
                let mut kept: Vec<usize> = Vec::new();
                for &i in &items {
                    if kept
                        .last()
                        .is_none_or(|&last| !apart(&rect(last), &rect(i)))
                    {
                        kept.push(i);
                    }
                }
                kept
            }
            _ => items.into_iter().step_by(2).collect(),
        };
        if items.len() < 3 || !overlaps(&items) {
            break;
        }
    }
    let last = *source.last().expect("three or more labels");
    if items.len() < 3 && !items.contains(&last) {
        if items.len() > 1 {
            items.pop();
        }
        items.push(last);
    }
    for i in source {
        if !items.contains(&i) {
            labels[i] = None;
        }
    }
}

/// `scaled`'s axis beside `plot`, `None` where it draws none.
pub(super) fn axis(layout: &mut Layout, scaled: &Scaled, plot: Rect) -> Option<Axis> {
    let channel = scaled.channel;
    let spec: &model::Axis = channel.axis.as_deref()?;
    let x = scaled.x;
    let default = if x { Side::Bottom } else { Side::Left };
    let side = spec
        .orient
        .filter(|side| side.is_horizontal() == x)
        .unwrap_or(default);
    let sign = match side {
        Side::Top | Side::Left => -1.0,
        Side::Bottom | Side::Right => 1.0,
    };
    let shift = spec.translate.map_or(0.0, |t| t - 0.5);
    let offset = spec.offset.unwrap_or(0.0);
    let along_shift = spec.position.unwrap_or(0.0) + shift;
    let base = shift
        + match side {
            Side::Bottom => plot.bottom() + offset,
            Side::Top => plot.y - offset,
            Side::Left => plot.x - offset,
            Side::Right => plot.right() + offset,
        };
    // A place along the run and out from the axis line.
    let point = |along: f32, out: f32| match x {
        true => [along, base + sign * out],
        false => [base + sign * out, along],
    };
    let bands = matches!(scaled.ticked, Ticked::Bands(..));
    let size = if x { plot.w } else { plot.h };
    let tick_offset = spec.tick_offset.unwrap_or(0.0);
    let (band, extra, tick_offset) = match spec.tick_band {
        Some(TickBand::Extent) => (1.0, true, 0.0),
        Some(TickBand::Center) => (0.5, false, tick_offset),
        None => (
            spec.band_position.unwrap_or(0.5),
            spec.tick_extra.unwrap_or(false),
            tick_offset,
        ),
    };
    let (range, ticks, log) = match scaled.ticked {
        Ticked::Bands(scale, labels) => {
            let indices: Vec<usize> = match &spec.values {
                Some(values) => values
                    .iter()
                    .filter_map(|value| labels.iter().position(|label| *label == named(value)))
                    .collect(),
                None => (0..labels.len()).collect(),
            };
            let width = scale.bandwidth();
            let ticks = indices
                .into_iter()
                .map(|i| {
                    let (value, text) = discrete(&labels[i], channel.kind, spec.format.as_ref());
                    let start = scale.at(i);
                    Tick {
                        value,
                        at: (start + band as f64 * width) as f32 + tick_offset,
                        middle: (start + width / 2.0) as f32 + tick_offset,
                        formatted: text.clone(),
                        text,
                    }
                })
                .collect::<Vec<_>>();
            let extra_at = extra.then(|| scale.at(0) as f32 + tick_offset);
            let range = [scale.range[0] as f32, scale.range[1] as f32];
            (range, (ticks, extra_at), false)
        }
        Ticked::Continuous(continuous) => {
            let (lo, hi) = (
                continuous.domain[0].min(continuous.domain[continuous.domain.len() - 1]),
                continuous.domain[0].max(continuous.domain[continuous.domain.len() - 1]),
            );
            let edges = match channel.scale.bins {
                Some(bins) if channel.kind == Kind::Quantitative => Some(bins.edges(lo, hi)),
                _ => None,
            };
            let given: Option<Vec<f64>> = spec.values.as_ref().map(|values| {
                values
                    .iter()
                    .filter_map(|value| match value {
                        Datum::Number(number) => Some(*number),
                        Datum::Text(_) => None,
                    })
                    .collect()
            });
            let log = matches!(continuous.transform, Transform::Log { .. });
            let count = spec.tick_count.unwrap_or(TickCount::Count(match &given {
                Some(values) => values.len() as f64,
                None if log => LOG_TICKS,
                None if edges.is_some() => f64::from((size / BIN_SPACING).ceil()),
                None => f64::from((size / TICK_SPACING).ceil()),
            }));
            let count = Count::of(
                continuous,
                count,
                spec.tick_min_step,
                edges.as_ref().map_or(0, Vec::len),
            )?;
            let values = match &given {
                Some(values) => scale::valid(continuous, values.clone(), count.number()),
                None => scale::values(continuous, count, edges.as_deref()),
            };
            let format = TickFormat::of(
                continuous,
                count.number(),
                spec.format.as_ref(),
                values.len(),
                given.is_some(),
            );
            let ticks = values
                .into_iter()
                .map(|v| {
                    let at = continuous.at(v) as f32 + tick_offset;
                    let text = format.label(v);
                    Tick {
                        value: Datum::Number(v),
                        at,
                        middle: at,
                        formatted: text.clone(),
                        text,
                    }
                })
                .collect();
            let range = [
                continuous.range[0] as f32,
                continuous.range[continuous.range.len() - 1] as f32,
            ];
            (range, (ticks, None), log)
        }
    };
    let (mut ticks, extra_at) = ticks;
    if let Some(relabel) = &spec.relabel {
        let last = ticks.len().saturating_sub(1).max(1) as f64;
        for (i, tick) in ticks.iter_mut().enumerate() {
            tick.text = relabel.0(&tick.value, &tick.text, i as f64 / last).to_string();
        }
    }
    // Each tick's style: the first case that holds sets each property it
    // sets, over the axis's own.
    let styles: Vec<TickStyle> = ticks
        .iter()
        .map(|tick| {
            spec.cases
                .iter()
                .filter(|case| case.test.0(&tick.value, &tick.formatted))
                .fold(TickStyle::default(), |set, case| set.over(&case.style))
                .over(&spec.style)
        })
        .chain(extra_at.map(|_| spec.style.clone()))
        .collect();
    let round = spec.tick_round.unwrap_or(true);
    let placed = |at: f32| {
        let at = at + along_shift;
        if round { at.round() } else { at }
    };
    let tick_places: Vec<f32> = ticks
        .iter()
        .map(|tick| tick.at)
        .chain(extra_at)
        .map(placed)
        .collect();

    let grid_shown = spec
        .style
        .grid
        .show
        .unwrap_or(!bands && channel.field().is_some() && channel.scale.bins.is_none());
    let grid = match grid_shown {
        true => tick_places
            .iter()
            .zip(&styles)
            .map(|(&at, style)| {
                let (from, to) = match x {
                    true => ([at, plot.y], [at, plot.bottom()]),
                    false => ([plot.x, at], [plot.right(), at]),
                };
                Rule {
                    from,
                    to,
                    stroke: stroke(&style.grid, Ink::Faint, LINE_WIDTH),
                }
            })
            .collect(),
        false => Vec::new(),
    };

    let mut rules = Vec::new();
    if spec.domain.show.unwrap_or(true) {
        rules.push(Rule {
            from: point(range[0] + along_shift, 0.0),
            to: point(range[1] + along_shift, 0.0),
            stroke: stroke(&spec.domain, Ink::Border, LINE_WIDTH),
        });
    }
    let ticks_shown = spec.style.tick.show.unwrap_or(true);
    let tick_size = |style: &TickStyle| match ticks_shown {
        true => style.size.unwrap_or(TICK_SIZE),
        false => 0.0,
    };
    if ticks_shown {
        rules.extend(tick_places.iter().zip(&styles).map(|(&at, style)| Rule {
            from: point(at, 0.0),
            to: point(at, tick_size(style)),
            stroke: stroke(&style.tick, Ink::Border, LINE_WIDTH),
        }));
    }

    let mut labels: Vec<Option<Label>> = Vec::new();
    if spec.labels.unwrap_or(true) {
        let flush = spec.label_flush.unwrap_or(
            (x && matches!(channel.kind, Kind::Quantitative | Kind::Temporal)).then_some(1.0),
        );
        let flush_offset = spec.label_flush_offset.unwrap_or(0.0);
        let (lo, hi) = (range[0].min(range[1]), range[0].max(range[1]));
        for (tick, style) in ticks.iter().zip(&styles) {
            if tick.text.is_empty() {
                labels.push(None);
                continue;
            }
            let lettering = &style.label;
            let out = tick_size(style) + style.label_padding.unwrap_or(LABEL_PADDING);
            let at = tick.middle + style.label_offset.unwrap_or(0.0) + along_shift;
            // Which end of the run it is flush with, if either.
            let end = flush.and_then(|threshold| {
                let (l, r) = ((tick.at - lo).abs(), (hi - tick.at).abs());
                match () {
                    _ if l < r && l <= threshold => Some(false),
                    _ if r <= threshold => Some(true),
                    _ => None,
                }
            });
            let label = layout.label(&tick.text, lettering, (1.0, false), Ink::Muted, LABEL_LIMIT);
            let (align, baseline, nudge) = match x {
                true => {
                    let align = match (lettering.align, end) {
                        (Some(align), _) => align,
                        (None, Some(false)) => Align::Left,
                        (None, Some(true)) => Align::Right,
                        (None, None) => Align::Center,
                    };
                    let baseline = lettering.baseline.unwrap_or(match side {
                        Side::Top => Baseline::Bottom,
                        _ => Baseline::Top,
                    });
                    let nudge = match (lettering.align, end) {
                        (None, Some(false)) => -flush_offset,
                        (None, Some(true)) => flush_offset,
                        _ => 0.0,
                    };
                    (align, baseline, nudge)
                }
                false => {
                    let align = lettering.align.unwrap_or(match side {
                        Side::Right => Align::Left,
                        _ => Align::Right,
                    });
                    let baseline = match (lettering.baseline, end) {
                        (Some(baseline), _) => baseline,
                        (None, Some(false)) => Baseline::Top,
                        (None, Some(true)) => Baseline::Bottom,
                        (None, None) => Baseline::Middle,
                    };
                    let nudge = match (lettering.baseline, end) {
                        (None, Some(false)) => -flush_offset,
                        (None, Some(true)) => flush_offset,
                        _ => 0.0,
                    };
                    (align, baseline, nudge)
                }
            };
            labels.push(Some(label.anchored(
                point(at + nudge, out),
                align,
                baseline,
            )));
        }
        let method = spec.label_overlap.unwrap_or(match (bands, &spec.format) {
            (true, _) if x => Overlap::Parity,
            (true, Some(Format::Time(_))) => Overlap::Parity,
            (true, _) => Overlap::Off,
            (false, _) if log => Overlap::Greedy,
            (false, _) => Overlap::Parity,
        });
        overlap(&mut labels, method, spec.label_separation.unwrap_or(0.0));
        if let Some(tolerance) = spec.label_bound {
            for slot in &mut labels {
                let outside = slot.as_ref().is_some_and(|label| {
                    let r = label.rect();
                    let (a, b) = match x {
                        true => (r.x, r.right()),
                        false => (r.y, r.bottom()),
                    };
                    a < lo + along_shift - tolerance || b > hi + along_shift + tolerance
                });
                if outside {
                    *slot = None;
                }
            }
        }
    }
    let labels: Vec<Label> = labels.into_iter().flatten().collect();

    // How far out from the axis line its ticks and labels reach.
    let reach = labels
        .iter()
        .map(|label| {
            let r = label.rect();
            match side {
                Side::Bottom => r.bottom() - base,
                Side::Top => base - r.y,
                Side::Left => base - r.x,
                Side::Right => r.right() - base,
            }
        })
        .fold(styles.iter().map(tick_size).fold(0.0, f32::max), f32::max);
    let reach = reach
        .min(spec.max_extent.unwrap_or(MAX_EXTENT))
        .max(spec.min_extent.unwrap_or(0.0));

    let title = scaled.title.as_ref().filter(|t| !t.is_empty()).map(|text| {
        let label = layout.label(
            text,
            &spec.title,
            (layout::GUIDE_TITLE, true),
            Ink::Text,
            0.0,
        );
        let padding = spec.title_padding.unwrap_or(TITLE_PADDING);
        let anchor = spec.title_anchor.map_or(0.5, Anchor::fraction);
        let lerp = range[0] + (range[1] - range[0]) * anchor + along_shift;
        let align = spec.title.align.unwrap_or(match spec.title_anchor {
            Some(Anchor::Start) => Align::Left,
            Some(Anchor::End) => Align::Right,
            _ => Align::Center,
        });
        // The axis group's origin, which `titleX` and `titleY` are from.
        let origin = match x {
            true => [plot.x + along_shift, base],
            false => [base, plot.y + along_shift],
        };
        let level = !x && spec.title.angle.is_none();
        let (at, align, baseline) = match (x, level) {
            (true, _) => (
                [lerp, base + sign * (reach + padding)],
                align,
                spec.title.baseline.unwrap_or(match side {
                    Side::Top => Baseline::Bottom,
                    _ => Baseline::Top,
                }),
            ),
            (false, false) => (
                [base + sign * (reach + padding), lerp],
                align,
                spec.title.baseline.unwrap_or(Baseline::Bottom),
            ),
            (false, true) => (
                [base + sign * reach, plot.y - padding],
                spec.title.align.unwrap_or(match side {
                    Side::Right => Align::Right,
                    _ => Align::Left,
                }),
                spec.title.baseline.unwrap_or(Baseline::Bottom),
            ),
        };
        let at = [
            spec.title_x.map_or(at[0], |dx| origin[0] + dx),
            spec.title_y.map_or(at[1], |dy| origin[1] + dy),
        ];
        label.anchored(at, align, baseline)
    });
    Some(Axis {
        side,
        grid,
        rules,
        labels,
        title,
        over: spec.zindex.map_or(
            scaled.mark == Mark::Rect && channel.kind.is_discrete(),
            |z| z > 0.0,
        ),
    })
}
