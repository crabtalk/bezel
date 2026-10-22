//! Vega-Lite's invalid data modes: which rows of a layer hold an invalid
//! value, and the chart with those rows taken out where its mode leaves them
//! out of marks and domains alike.

use crate::{
    data::{Column, Data},
    model::{Chart, Domain, Encode, Invalid, Kind, Layer, ScaleType},
    plan::{
        domain::Discrete,
        scales::{self, Role},
    },
};
use gpui::SharedString;
use std::borrow::Cow;

/// The channels of an encoding with a scale of their own, each with its
/// role: `as_ref` and `Encode::channel` to read them, `as_mut` and
/// `Encode::channel_mut` to change them.
macro_rules! scaled {
    ($e:expr, $as:ident, $channel:path) => {
        [
            ($e.x.$as().and_then($channel), Role::X),
            ($e.y.$as().and_then($channel), Role::Y),
            ($e.x_offset.$as().and_then($channel), Role::Offset),
            ($e.y_offset.$as().and_then($channel), Role::Offset),
            ($e.theta.$as().and_then($channel), Role::Theta),
            ($e.radius.$as().and_then($channel), Role::Radius),
            ($e.color.$as().and_then($channel), Role::Color),
            ($e.fill.$as().and_then($channel), Role::Color),
            ($e.stroke.$as().and_then($channel), Role::Color),
            ($e.opacity.$as().and_then($channel), Role::Amount),
            ($e.fill_opacity.$as().and_then($channel), Role::Amount),
            ($e.stroke_opacity.$as().and_then($channel), Role::Amount),
            ($e.stroke_width.$as().and_then($channel), Role::Amount),
            ($e.angle.$as().and_then($channel), Role::Amount),
            ($e.size.$as().and_then($channel), Role::Size),
            ($e.shape.$as().and_then($channel), Role::Symbol),
            ($e.stroke_dash.$as().and_then($channel), Role::Symbol),
        ]
    };
}

/// `chart` as its layers' modes draw it: the rows its marks leave out taken
/// out, and its domains held to the rows they take in where those differ.
pub(super) fn prepare(chart: &Chart) -> Cow<'_, Chart> {
    let mut prepared: Option<Chart> = None;
    for (index, layer) in chart.layers.iter().enumerate() {
        let Some(invalid) = rows(layer) else {
            continue;
        };
        let changed = match (layer.invalid(), layer.mark.is_path()) {
            (Invalid::Filter, _) | (Invalid::BreakPathsFilterDomains, false) => Layer {
                data: layer.data.select(&valid(&invalid)),
                ..layer.clone()
            },
            (Invalid::BreakPathsFilterDomains, true) => held(layer, &invalid, false),
            (Invalid::BreakPathsShowDomains, false) => held(layer, &invalid, true),
            _ => continue,
        };
        prepared.get_or_insert_with(|| chart.clone()).layers[index] = changed;
    }
    match prepared {
        Some(chart) => Cow::Owned(chart),
        None => Cow::Borrowed(chart),
    }
}

/// Each row's whether it holds an invalid value, one that is not a finite
/// number, on a channel that counts: a quantitative or temporal field on a
/// scale whose domain is continuous. `None` where no row does.
pub(super) fn rows(layer: &Layer) -> Option<Vec<bool>> {
    let mut invalid = vec![false; layer.data.len()];
    for (channel, role) in scaled!(layer.encoding, as_ref, Encode::channel) {
        let Some((channel, field)) = channel.and_then(|c| Some((c, c.field()?))) else {
            continue;
        };
        let kind = scales::scale_type(channel, role, layer.mark, false);
        if matches!(channel.kind, Kind::Nominal | Kind::Ordinal)
            || kind.is_discrete()
            || kind == ScaleType::BinOrdinal
        {
            continue;
        }
        for (row, value) in layer.data.numbers(field).into_iter().enumerate() {
            invalid[row] |= !value.is_finite();
        }
    }
    invalid.contains(&true).then_some(invalid)
}

/// The rows a path breaks at.
pub(super) fn breaks(layer: &Layer) -> Option<Vec<bool>> {
    match (layer.invalid(), layer.mark.is_path()) {
        (Invalid::BreakPathsFilterDomains | Invalid::BreakPathsShowDomains, true) => rows(layer),
        _ => None,
    }
}

/// Whether a layer draws an invalid value as [`zero_or_min`] of its scale.
pub(super) fn shown(layer: &Layer) -> bool {
    layer.invalid() == Invalid::Show
}

/// Vega-Lite's `zeroOrMin`: zero where `domain` spans it, its first value
/// otherwise.
pub(super) fn zero_or_min(domain: &[f64]) -> f64 {
    let (first, last) = (domain[0], domain[domain.len() - 1]);
    match first.min(last) <= 0.0 && 0.0 <= first.max(last) {
        true => 0.0,
        false => first,
    }
}

fn valid(invalid: &[bool]) -> Vec<usize> {
    (0..invalid.len()).filter(|&row| !invalid[row]).collect()
}

/// `layer` with each channel's domain held to its rows as they stand, and
/// then, with `drop`, its `invalid` rows taken out; without, held to the
/// rows not `invalid`, `x2` and `y2` missing in the others. A discrete
/// domain is held to the rows' values, a continuous one to their numbers
/// and their far channel's.
fn held(layer: &Layer, invalid: &[bool], drop: bool) -> Layer {
    let valid = valid(invalid);
    let rest = match drop {
        true => layer.data.clone(),
        false => layer.data.select(&valid),
    };
    let e = &layer.encoding;
    let far = |role: Role| -> Option<&SharedString> {
        let far = match role {
            Role::X => e.x2.as_ref()?.channel()?,
            Role::Y => e.y2.as_ref()?.channel()?,
            Role::Theta => e.theta2.as_ref()?.channel()?,
            Role::Radius => e.radius2.as_ref()?.channel()?,
            _ => return None,
        };
        far.field()
    };
    let mut out = layer.clone();
    for (channel, role) in scaled!(out.encoding, as_mut, Encode::channel_mut) {
        let Some(channel) = channel else {
            continue;
        };
        let Some(field) = channel.field().cloned() else {
            continue;
        };
        let kind = scales::scale_type(channel, role, layer.mark, false);
        let fields = match channel.scale.domain.take() {
            None if channel.scale.bins.is_some() => continue,
            None if scales::listed(kind, channel) => {
                channel.scale.domain = channel
                    .column(&rest)
                    .map(|column| Domain::Values(Discrete::of(&column, &channel.sort).labels));
                continue;
            }
            None => vec![field],
            Some(Domain::Unaggregated(fields)) => fields.to_vec(),
            domain => {
                channel.scale.domain = domain;
                continue;
            }
        };
        let numbers: Vec<f64> = fields
            .iter()
            .chain(far(role).filter(|_| drop))
            .flat_map(|field| rest.numbers(field))
            .collect();
        channel.scale.domain = Some(Domain::Data(numbers.into()));
    }
    out.data = match drop {
        true => layer.data.select(&valid),
        false => [&e.x2, &e.y2]
            .into_iter()
            .filter_map(|far| far.as_ref()?.channel()?.field())
            .fold(layer.data.clone(), |data, field| {
                let values = missing(&layer.data, field, invalid);
                data.adopt(&layer.data, field.clone(), values)
            }),
    };
    out
}

/// `field`'s numbers, missing in the `invalid` rows.
fn missing(data: &Data, field: &str, invalid: &[bool]) -> Column {
    Column::Number(
        data.numbers(field)
            .into_iter()
            .zip(invalid)
            .map(|(value, &invalid)| if invalid { f64::NAN } else { value })
            .collect(),
    )
}
