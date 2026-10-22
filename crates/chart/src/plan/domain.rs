//! What values a channel takes: a discrete column's values in order, and
//! the chart's colours.

use crate::{
    data::{Column, Name, Text},
    model::{Chart, Domain, Encode, Mark, Range, Scale, ScaleType, Sort},
    plan::{
        Color, Ink, invalid,
        scales::{self, ColorScale, Role},
    },
};
use gpui::{Hsla, SharedString};
use js::number::Number as JsNumber;
use std::{collections::HashMap, sync::Arc};

/// A missing code in a discrete column.
pub(super) const MISSING: u32 = u32::MAX;

/// A column's distinct values in order, and each row's index into them.
pub(super) struct Discrete {
    pub(super) labels: Vec<SharedString>,
    pub(super) codes: Arc<[u32]>,
}

impl Discrete {
    /// Null, undefined, and a number column's `NaN` as null, are values of
    /// their own, first in ascending order and last in descending order as
    /// Vega sorts them, in the order met.
    pub(super) fn of(column: &Column, sort: &Sort) -> Self {
        let text = |text: &Text| {
            let labels = text.names.iter().map(|name| name.to_string().into());
            let nullish = text.names.iter().map(|name| !matches!(name, Name::Text(_)));
            (
                labels.collect(),
                text.codes.clone(),
                None,
                nullish.collect(),
            )
        };
        let (labels, codes, numbers, nullish): (Vec<SharedString>, Arc<[u32]>, _, Vec<bool>) =
            match column {
                Column::Text(column) => text(column),
                Column::Value(values) => text(&Text::from(&values[..])),
                Column::Number(values) => {
                    let mut index = HashMap::new();
                    let mut distinct = Vec::new();
                    let codes = values
                        .iter()
                        .map(|&v| {
                            // Every `NaN` is one value.
                            let bits = if v.is_nan() { f64::NAN } else { v }.to_bits();
                            *index.entry(bits).or_insert_with(|| {
                                distinct.push(v);
                                distinct.len() as u32 - 1
                            })
                        })
                        .collect();
                    let labels = distinct
                        .iter()
                        .map(|&v| match v.is_nan() {
                            true => Name::Null.to_string().into(),
                            false => JsNumber(v).to_string().into(),
                        })
                        .collect();
                    let nullish = distinct.iter().map(|v| v.is_nan()).collect();
                    (labels, codes, Some(distinct), nullish)
                }
            };
        let mut order: Vec<usize> = (0..labels.len()).collect();
        match sort {
            Sort::Data => return Self { labels, codes },
            Sort::Ascending | Sort::Descending => {
                let ascending = |a: usize, b: usize| match (nullish[a], nullish[b], &numbers) {
                    (true, _, _) | (_, true, _) => nullish[b].cmp(&nullish[a]),
                    (.., Some(values)) => values[a].total_cmp(&values[b]),
                    (.., None) => labels[a].cmp(&labels[b]),
                };
                match sort {
                    Sort::Descending => order.sort_by(|&a, &b| ascending(b, a)),
                    _ => order.sort_by(|&a, &b| ascending(a, b)),
                }
            }
            Sort::Explicit(listed) => {
                let rank = |index: &usize| {
                    listed
                        .iter()
                        .position(|name| *name == labels[*index])
                        .unwrap_or(listed.len())
                };
                order.sort_by_key(rank);
            }
        }
        let mut rank = vec![0u32; labels.len()];
        for (new, &old) in order.iter().enumerate() {
            rank[old] = new as u32;
        }
        Self {
            labels: order.iter().map(|&old| labels[old].clone()).collect(),
            codes: codes
                .iter()
                .map(|&code| match code {
                    MISSING => MISSING,
                    code => rank[code as usize],
                })
                .collect(),
        }
    }

    /// Its values as `scale` lists them, when it lists them: rows with
    /// others are missing.
    pub(super) fn within(self, scale: &Scale) -> Self {
        let Some(Domain::Values(listed)) = &scale.domain else {
            return self;
        };
        let place: Vec<u32> = self
            .labels
            .iter()
            .map(|label| {
                listed
                    .iter()
                    .position(|name| name == label)
                    .map_or(MISSING, |at| at as u32)
            })
            .collect();
        Self {
            labels: listed.clone(),
            codes: self
                .codes
                .iter()
                .map(|&code| match code {
                    MISSING => MISSING,
                    code => place[code as usize],
                })
                .collect(),
        }
    }

    /// Each row's code into `labels`, a domain shared with other layers,
    /// which takes in this one's values it has not met.
    pub(super) fn into_shared(
        self,
        labels: &mut Vec<SharedString>,
        index: &mut HashMap<SharedString, u32>,
    ) -> Vec<u32> {
        let global: Vec<u32> = self
            .labels
            .into_iter()
            .map(|label| {
                *index.entry(label.clone()).or_insert_with(|| {
                    labels.push(label);
                    labels.len() as u32 - 1
                })
            })
            .collect();
        self.codes
            .iter()
            .map(|&code| match code {
                MISSING => MISSING,
                code => global[code as usize],
            })
            .collect()
    }
}

/// The lowest and highest of `values` that are numbers, `(0, 0)` for none.
pub(super) fn extent(values: impl IntoIterator<Item = f64>) -> (f64, f64) {
    let (lo, hi) = values
        .into_iter()
        .filter(|v| !v.is_nan())
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), v| {
            (lo.min(v), hi.max(v))
        });
    if lo > hi { (0.0, 0.0) } else { (lo, hi) }
}

/// The chart's colours: every colour field's values across the layers, in
/// the order met, or every value a continuous colour field takes; then one
/// for each layer with no colour field: the theme's text for a rule or
/// text, the next categorical colour for any other.
pub(super) struct Colors {
    pub(super) colors: Vec<Color>,
    pub(super) layers: Vec<LayerColor>,
    /// A continuous or discretizing colour field's scale.
    pub(super) palette: Option<ColorScale>,
}

pub(super) enum LayerColor {
    /// Each row's colour.
    Field(Vec<u32>),
    Fixed(usize),
}

impl Colors {
    pub(super) fn of(chart: &Chart) -> Option<Self> {
        let first = chart
            .layers
            .iter()
            .find_map(|layer| Some((layer.mark, layer.encoding.color.as_ref()?.channel()?)));
        let mut colors: Vec<Color> = Vec::new();
        let mut layers: Vec<Option<LayerColor>> = Vec::with_capacity(chart.layers.len());
        let mut palette = None;
        let named = first.is_some_and(|(_, channel)| {
            channel.scale.kind == Some(ScaleType::Identity)
                || matches!(channel.scale.range, Some(Range::Field(_)))
        });
        if let Some((_, channel)) = first.filter(|_| named) {
            // Each value its own colour, or its rows' in the range field.
            let mut labels = Vec::new();
            let mut index = HashMap::new();
            let mut inks: Vec<Option<Ink>> = Vec::new();
            for layer in &chart.layers {
                let Some(Encode::Scaled(color)) = &layer.encoding.color else {
                    layers.push(None);
                    continue;
                };
                let rows = scales::named_inks(color, layer)??;
                let domain =
                    Discrete::of(&color.column(&layer.data)?, &color.sort).within(&channel.scale);
                let codes = domain.into_shared(&mut labels, &mut index);
                inks.resize(labels.len(), None);
                for (row, &code) in codes.iter().enumerate() {
                    if code != MISSING && inks[code as usize].is_none() {
                        inks[code as usize] = rows[row];
                    }
                }
                layers.push(Some(LayerColor::Field(codes)));
            }
            colors = labels
                .into_iter()
                .zip(inks)
                .map(|(name, ink)| Color {
                    name: Some(name),
                    ink: ink.unwrap_or(Ink::Fixed(Hsla::transparent_black())),
                })
                .collect();
        } else if let Some((mark, channel)) = first {
            let kind = scales::scale_type(channel, Role::Color, mark, false);
            if scales::listed(kind, channel) {
                let mut labels = Vec::new();
                let mut index = HashMap::new();
                for layer in &chart.layers {
                    let Some(Encode::Scaled(color)) = &layer.encoding.color else {
                        layers.push(None);
                        continue;
                    };
                    let domain = Discrete::of(&color.column(&layer.data)?, &color.sort)
                        .within(&channel.scale);
                    let codes = domain.into_shared(&mut labels, &mut index);
                    layers.push(Some(LayerColor::Field(codes)));
                }
                let discrete = ColorScale::of(channel, mark, Some(labels.len()), &[])?;
                colors = labels
                    .into_iter()
                    .enumerate()
                    .map(|(i, name)| {
                        Some(Color {
                            name: Some(name),
                            ink: discrete.discrete(i)?,
                        })
                    })
                    .collect::<Option<_>>()?;
            } else {
                let mut columns = Vec::with_capacity(chart.layers.len());
                let mut spanned = Vec::new();
                for layer in &chart.layers {
                    columns.push(match &layer.encoding.color {
                        Some(Encode::Scaled(color)) => {
                            spanned.extend(color.spanned(&layer.data)?);
                            Some((color.numbers(&layer.data)?, invalid::shown(layer)))
                        }
                        _ => None,
                    });
                }
                let values: Vec<f64> = spanned.iter().flat_map(|v| v.iter().copied()).collect();
                let scale = ColorScale::of(channel, mark, None, &values)?;
                let mut index: HashMap<u64, u32> = HashMap::new();
                for column in columns {
                    let Some((values, shown)) = column else {
                        layers.push(None);
                        continue;
                    };
                    let codes = values
                        .iter()
                        .map(|&v| {
                            let v = if shown { scale.shown(v) } else { v };
                            let Some(ink) = scale.value(v) else {
                                return MISSING;
                            };
                            *index.entry(v.to_bits()).or_insert_with(|| {
                                colors.push(Color {
                                    name: Some(channel.kind.describe(v).into()),
                                    ink,
                                });
                                colors.len() as u32 - 1
                            })
                        })
                        .collect();
                    layers.push(Some(LayerColor::Field(codes)));
                }
                palette = Some(scale);
            }
        } else {
            layers.extend(chart.layers.iter().map(|_| None));
        }
        let layers = layers
            .into_iter()
            .zip(&chart.layers)
            .map(|(color, layer)| {
                color.unwrap_or_else(|| {
                    let ink = match (&layer.encoding.color, layer.mark) {
                        (Some(Encode::Value(fixed)), _) => Ink::Fixed(*fixed),
                        (_, Mark::Rule | Mark::Text) => Ink::Text,
                        _ => Ink::Category(colors.len()),
                    };
                    colors.push(Color { name: None, ink });
                    LayerColor::Fixed(colors.len() - 1)
                })
            })
            .collect();
        Some(Self {
            colors,
            layers,
            palette,
        })
    }

    pub(super) fn of_row(&self, layer: usize, row: usize) -> Option<usize> {
        match &self.layers[layer] {
            LayerColor::Fixed(color) => Some(*color),
            LayerColor::Field(codes) => (codes[row] != MISSING).then_some(codes[row] as usize),
        }
    }
}
