//! What values a channel takes: a discrete column's values in order, a
//! continuous one's ticks, and the chart's colours.

use std::{collections::HashMap, sync::Arc};

use gpui::SharedString;

use crate::{
    data::Column,
    model::{Channel, Chart, Encode, Kind, Mark, Scale, Sort},
    plan::{Color, Ink},
    scale::{self, Ticks},
};

/// A missing code in a discrete column.
pub(super) const MISSING: u32 = u32::MAX;

/// A column's distinct values in order, and each row's index into them.
pub(super) struct Discrete {
    pub(super) labels: Vec<SharedString>,
    pub(super) codes: Arc<[u32]>,
}

impl Discrete {
    pub(super) fn of(column: &Column, sort: &Sort) -> Self {
        let (labels, codes, numbers): (Vec<SharedString>, Arc<[u32]>, Option<Vec<f64>>) =
            match column {
                Column::Text(text) => (text.names.to_vec(), text.codes.clone(), None),
                Column::Number(values) => {
                    let mut index = HashMap::new();
                    let mut distinct = Vec::new();
                    let codes = values
                        .iter()
                        .map(|&v| match v.is_nan() {
                            true => MISSING,
                            false => *index.entry(v.to_bits()).or_insert_with(|| {
                                distinct.push(v);
                                distinct.len() as u32 - 1
                            }),
                        })
                        .collect();
                    let labels = distinct.iter().map(|&v| scale::value(v).into()).collect();
                    (labels, codes, Some(distinct))
                }
            };
        let mut order: Vec<usize> = (0..labels.len()).collect();
        match sort {
            Sort::Data => return Self { labels, codes },
            Sort::Ascending | Sort::Descending => {
                order.sort_by(|&a, &b| match &numbers {
                    Some(values) => values[a].total_cmp(&values[b]),
                    None => labels[a].cmp(&labels[b]),
                });
                if *sort == Sort::Descending {
                    order.reverse();
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

/// Ticks for a quantitative or temporal channel whose data spans `lo..=hi`.
pub(super) fn continuous(channel: &Channel, lo: f64, hi: f64, count: usize) -> Ticks {
    let Scale { zero, domain, nice } = channel.scale;
    match (channel.kind, domain) {
        (Kind::Temporal, Some([lo, hi])) => scale::temporal(lo, hi, count),
        (Kind::Temporal, None) => scale::temporal(lo, hi, count),
        (_, Some([lo, hi])) => scale::linear(lo, hi, count, false),
        (_, None) if zero => scale::linear(lo.min(0.0), hi.max(0.0), count, nice),
        (_, None) => scale::linear(lo, hi, count, nice),
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
/// the order met, then one for each layer with no colour field: the theme's
/// text for a rule or text, the next categorical colour for any other.
pub(super) struct Colors {
    pub(super) colors: Vec<Color>,
    pub(super) layers: Vec<LayerColor>,
}

pub(super) enum LayerColor {
    /// Each row's colour.
    Field(Vec<u32>),
    Fixed(usize),
}

impl Colors {
    pub(super) fn of(chart: &Chart) -> Option<Self> {
        let mut labels = Vec::new();
        let mut index = HashMap::new();
        let mut layers = Vec::with_capacity(chart.layers.len());
        for layer in &chart.layers {
            let color = match &layer.encoding.color {
                Some(Encode::Scaled(color)) => color,
                _ => {
                    layers.push(None);
                    continue;
                }
            };
            if !color.kind.is_discrete() {
                return None;
            }
            let domain = Discrete::of(&color.column(&layer.data)?, &color.sort);
            let codes = domain.into_shared(&mut labels, &mut index);
            layers.push(Some(LayerColor::Field(codes)));
        }
        let mut colors: Vec<Color> = labels
            .into_iter()
            .enumerate()
            .map(|(i, name)| Color {
                name: Some(name),
                ink: Ink::Category(i),
            })
            .collect();
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
        Some(Self { colors, layers })
    }

    pub(super) fn of_row(&self, layer: usize, row: usize) -> Option<usize> {
        match &self.layers[layer] {
            LayerColor::Fixed(color) => Some(*color),
            LayerColor::Field(codes) => (codes[row] != MISSING).then_some(codes[row] as usize),
        }
    }
}
