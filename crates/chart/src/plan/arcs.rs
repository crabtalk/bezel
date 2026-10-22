//! Pie and donut slices: each row's `theta` stacked round its range, once
//! round from twelve o'clock by default, or overlapping from its start
//! without a stack, or ranged to `theta2`, or one band per value; `radius`
//! and `radius2` their outer and inner edges through the radius scale, and
//! the pad angle taken out between them. A text layer over
//! them stands each row's text at its own slice's middle angle, `radius` out
//! from the centre. Rows stack into one pie per centre. A layer's centre is
//! the middle of the chart, or its `x` and `y` in pixels from the top-left
//! corner of the room under the legends; over axes, each row's place on the
//! x and y scales.

use crate::{
    data::Column,
    model::{Channel, Chart, Encode, Layer, Mark, Position, ScaleType, Stack},
    plan::{
        self, Body, Font, Hit, Hover, Link, Marks, Rect, Series, Slice, Text,
        domain::{self, Colors, Discrete},
        look::{self, Groups, Look},
        scales::{self, Context, Role},
    },
};
use d3::{scale::Band, shape::curve::Curve};
use gpui::SharedString;
use std::{collections::HashMap, f32::consts::TAU};

/// A radius for each row, in pixels: a value, or a field through its
/// radius scale, `scale`'s where it shares one, from zero to `outer` by
/// default. `NaN` for a row with none.
fn radii(
    encode: Option<&Encode<f32>>,
    scale: Option<&Channel>,
    layer: &Layer,
    outer: f32,
    default: f32,
) -> Option<Vec<f32>> {
    let rows = layer.data.len();
    Some(match encode {
        None => vec![default; rows],
        Some(Encode::Value(pixels)) => vec![*pixels; rows],
        Some(Encode::Scaled(channel)) => {
            let values = channel.numbers(&layer.data)?;
            let scale_channel = scale.unwrap_or(channel);
            let kind = scales::scale_type(scale_channel, Role::Radius, Mark::Arc, false);
            if !kind.is_continuous() {
                return None;
            }
            let spanned = scale_channel.spanned(&layer.data)?;
            let extent = domain::extent(spanned.iter().flat_map(|v| v.iter().copied()));
            let range = scales::numbers(&scale_channel.scale, [0.0, outer])?;
            let context = Context {
                role: Role::Radius,
                mark: Mark::Arc,
                orient: None,
                both_measures: false,
            };
            let radius = scales::continuous(scale_channel, kind, &context, extent, range);
            values.iter().map(|&v| radius.at(v) as f32).collect()
        }
    })
}

/// Where a theta channel's range starts and ends: its own, or once round
/// clockwise from twelve o'clock.
fn turn(theta: &Channel) -> Option<(f32, f32)> {
    let range = scales::numbers(&theta.scale, [0.0, TAU])?;
    let (start, end) = (range[0] as f32, range[range.len() - 1] as f32);
    Some(match theta.scale.reverse == Some(true) {
        true => (end, start),
        false => (start, end),
    })
}

/// Slices in `room`, what the title and legends leave.
pub(super) fn arcs(chart: &Chart, colors: &Colors, room: Rect) -> Option<Body> {
    let outer = (room.w.min(room.h) / 2.0).max(plan::MIN_PLOT / 2.0);
    let center = [room.x + room.w / 2.0, room.y + room.h / 2.0];
    let plot = Rect {
        x: center[0] - outer,
        y: center[1] - outer,
        w: outer * 2.0,
        h: outer * 2.0,
    };
    let mut series = Vec::new();
    let mut hover = Hover::default();
    let mut links = Vec::new();
    for (index, layer) in chart.layers.iter().enumerate() {
        // A position in pixels, or the middle without one.
        let at =
            |encode: &Option<Encode<Position>>, middle: f32, start: f32, size: f32| match encode {
                None => Some(middle),
                Some(Encode::Value(position)) => Some(start + position.pixels(size)),
                Some(Encode::Scaled(_)) => None,
            };
        let e = &layer.encoding;
        let center = [
            at(&e.x, center[0], room.x, room.w)?,
            at(&e.y, center[1], room.y, room.h)?,
        ];
        polar(
            layer,
            index,
            colors,
            &|_| Some(center),
            outer,
            room,
            (&mut series, &mut hover, &mut links),
        )?;
    }
    Some((plot, Vec::new(), series, hover, links))
}

/// What a layer adds to the body: series, hover and links.
type Out<'a> = (&'a mut Vec<Series>, &'a mut Hover, &'a mut Vec<Link>);

/// An arc layer's slices, or a text layer's texts at them, each row's pie
/// at `center`, `outer` the radius of a pie with no `radius`. A row with no
/// centre is not drawn. With `clip`, its marks are cut off at `bounds`.
pub(super) fn polar(
    layer: &Layer,
    index: usize,
    colors: &Colors,
    center: &dyn Fn(usize) -> Option<[f32; 2]>,
    outer: f32,
    bounds: Rect,
    (series, hover, links): Out,
) -> Option<()> {
    let groups = Groups::of(layer, index, colors)?;
    let look = Look::of(layer, 0.0)?;
    let slices = slices(layer, &groups, center, outer)?;
    let ink = |group: usize| colors.colors[groups.groups[group].0].ink;
    let new_series = |group: usize, marks: Marks| {
        let (color, first) = groups.groups[group];
        Series {
            name: colors.colors[color].name.clone(),
            color,
            mark: layer.mark,
            marks,
            style: look.style(first, ink(group)),
            curve: Curve::Linear,
            clip: layer.properties.clip.then_some(bounds),
        }
    };
    if layer.mark == Mark::Text {
        // Each text at its slice's middle angle, its radius out.
        let p = &layer.properties;
        let radius = radii(layer.encoding.radius.as_ref(), None, layer, outer, 0.0)?;
        let mut texts: Vec<Vec<Text>> = vec![Vec::new(); groups.len()];
        for (group, slice, _, row) in &slices {
            let text = look.text.at(*row);
            if text.is_empty() {
                continue;
            }
            let angle = (slice.start + slice.end) / 2.0;
            let out = radius[*row] + p.radius_offset;
            let Some(center) = center(*row) else {
                continue;
            };
            let at = [
                center[0] + out * angle.sin() + p.dx,
                center[1] - out * angle.cos() + p.dy,
            ];
            let size = look.size.at(*row);
            if let Some(href) = look.href(*row) {
                links.push(Link {
                    hit: Hit::Disc(at, size),
                    href,
                });
            }
            texts[*group].push(Text { at, text, size });
        }
        for (group, texts) in texts.into_iter().enumerate() {
            if !texts.is_empty() {
                series.push(new_series(group, Marks::Texts(texts, Font::of(layer))));
            }
        }
        return Some(());
    }
    let mut by_group: Vec<Vec<Slice>> = vec![Vec::new(); groups.len()];
    for (group, slice, _, row) in &slices {
        by_group[*group].push(*slice);
        if let Some(href) = look.href(*row) {
            links.push(Link {
                hit: Hit::Slice(*slice),
                href,
            });
        }
    }
    for (group, slices) in by_group.into_iter().enumerate() {
        if !slices.is_empty() {
            series.push(new_series(group, Marks::Slices(slices)));
        }
    }
    hover.slices.extend(
        slices
            .into_iter()
            .map(|(group, slice, readout, _)| (groups.groups[group].0, slice, readout)),
    );
    Some(())
}

/// Each of `layer`'s slices, its series and its read-out, in drawing
/// order, and its row.
fn slices(
    layer: &Layer,
    groups: &Groups,
    center: &dyn Fn(usize) -> Option<[f32; 2]>,
    outer: f32,
) -> Option<Vec<(usize, Slice, SharedString, usize)>> {
    let e = &layer.encoding;
    let theta = e.theta.as_ref()?.channel()?;
    let radius_scale = e.radius.as_ref().and_then(Encode::channel);
    let radius = radii(e.radius.as_ref(), None, layer, outer, outer)?;
    let inner = radii(e.radius2.as_ref(), radius_scale, layer, outer, 0.0)?;
    let ranks = look::ranks(layer)?;
    let p = &layer.properties;
    // `theta2Offset` turns a given `theta2`; without one the end turns with
    // the start.
    let turn2 = match e.theta2 {
        Some(_) => p.theta2_offset,
        None => p.theta_offset,
    };
    let inset = match e.radius2 {
        Some(_) => p.radius2_offset,
        None => 0.0,
    };
    let slice = |row: usize, start: f32, end: f32| Slice {
        center: center(row).unwrap_or_default(),
        radius: radius[row] + p.radius_offset,
        inner: inner[row] + inset,
        start: start + p.theta_offset,
        end: end + turn2,
        pad: p.pad_angle,
        corner: p.corner_radius[0].unwrap_or(0.0),
    };

    // Each slice, its series and its read-out, in drawing order.
    let mut slices: Vec<(usize, Slice, SharedString, usize)> = Vec::new();
    // With no scale, each value is an angle in radians.
    let identity = theta.scale.kind == Some(ScaleType::Identity);
    if identity && theta.kind.is_discrete() {
        return None;
    }
    let (from, to) = turn(theta)?;
    let around = |share: f64| from + (to - from) * share as f32;
    if theta.kind.is_discrete() {
        let domain = Discrete::of(&theta.column(&layer.data)?, &theta.sort).within(&theta.scale);
        let band = Band::new(domain.labels.len(), [f64::from(from), f64::from(to)]);
        let readout = |row: usize| -> SharedString {
            match radius_scale.and_then(|c| Some((c, c.column(&layer.data)?))) {
                Some((channel, Column::Number(values))) => {
                    channel.kind.describe(values[row]).into()
                }
                _ => domain.labels[domain.codes[row] as usize].clone(),
            }
        };
        for (row, &code) in domain.codes.iter().enumerate() {
            if code == domain::MISSING
                || groups.codes[row] == domain::MISSING
                || center(row).is_none()
            {
                continue;
            }
            let start = band.at(code as usize) as f32;
            let s = slice(row, start, start + band.bandwidth() as f32);
            slices.push((groups.codes[row] as usize, s, readout(row), row));
        }
    } else {
        let values = theta.numbers(&layer.data)?;
        if let Some(theta2) = e.theta2.as_ref() {
            let theta2 = theta2.channel()?.numbers(&layer.data)?;
            let spanned = theta.spanned(&layer.data)?;
            let extent = domain::extent(
                spanned
                    .iter()
                    .flat_map(|v| v.iter())
                    .chain(theta2.iter())
                    .copied(),
            );
            let kind = scales::scale_type(theta, Role::Theta, Mark::Arc, false);
            let context = Context {
                role: Role::Theta,
                mark: Mark::Arc,
                orient: None,
                both_measures: false,
            };
            let scale = scales::continuous(
                theta,
                kind,
                &context,
                extent,
                vec![f64::from(from), f64::from(to)],
            );
            let angle = |v: f64| match identity {
                true => v as f32,
                false => scale.at(v) as f32,
            };
            for row in 0..values.len() {
                let (v, v2) = (values[row], theta2[row]);
                if groups.codes[row] == domain::MISSING
                    || v.is_nan()
                    || v2.is_nan()
                    || center(row).is_none()
                {
                    continue;
                }
                let readout = format!("{} – {}", theta.kind.describe(v), theta.kind.describe(v2));
                let s = slice(row, angle(v.min(v2)), angle(v.max(v2)));
                slices.push((groups.codes[row] as usize, s, readout.into(), row));
            }
        } else {
            // Each row its own slice, by `order`, or series by series,
            // stacked round its own centre's pie.
            let pie = |row: usize| center(row).map(|[x, y]| (x.to_bits(), y.to_bits()));
            let overlap = theta.stack == Stack::Off;
            let mut rows: Vec<usize> = (0..values.len())
                .filter(|&row| {
                    let value = values[row];
                    let drawn = match identity {
                        true => !value.is_nan(),
                        false => value > 0.0,
                    };
                    groups.codes[row] != domain::MISSING && drawn && pie(row).is_some()
                })
                .collect();
            match &ranks {
                Some(ranks) => rows.sort_by_key(|&row| ranks[row]),
                None => rows.sort_by_key(|&row| groups.codes[row]),
            }
            if identity {
                // Vega's stack of the angles themselves: up from zero, and
                // a negative one down from it.
                let mut ends: HashMap<(u32, u32), (f64, f64)> = HashMap::new();
                for row in rows {
                    let value = values[row];
                    let (up, down) = ends.entry(pie(row)?).or_insert((0.0, 0.0));
                    let start = match (overlap, value < 0.0) {
                        (true, _) => 0.0,
                        (false, true) => std::mem::replace(down, *down + value),
                        (false, false) => std::mem::replace(up, *up + value),
                    };
                    let readout = theta.kind.describe(value);
                    let s = slice(row, start as f32, (start + value) as f32);
                    slices.push((groups.codes[row] as usize, s, readout.into(), row));
                }
                return Some(slices);
            }
            // Each pie's total, then how far round it the next slice starts;
            // unstacked, each slice from the start to its share of the
            // largest.
            let mut pies: HashMap<(u32, u32), (f64, f64)> = HashMap::new();
            for &row in &rows {
                let (total, _) = pies.entry(pie(row)?).or_insert((0.0, 0.0));
                *total = match overlap {
                    true => total.max(values[row]),
                    false => *total + values[row],
                };
            }
            for row in rows {
                let (total, start) = pies.get_mut(&pie(row)?)?;
                let value = values[row];
                let end = *start + value / *total;
                let share = value / *total * 100.0;
                let readout = format!("{} ({share:.1}%)", theta.kind.describe(value));
                let s = slice(row, around(*start), around(end));
                slices.push((groups.codes[row] as usize, s, readout.into(), row));
                if !overlap {
                    *start = end;
                }
            }
        }
    }

    Some(slices)
}
