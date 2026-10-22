//! A discrete scale the layers of a chart share: Vega-Lite's merge of their
//! domains. Layers with the same field over the same rows share one domain,
//! sorted as they sort it; where they sort it differently, by the one op
//! other than `min` among sorts all by an op, and otherwise as `true` does.
//! Layers with other fields or rows union their domains, sorted by the one
//! `count`, `min` or `max` among their sorts, any other sort taken as
//! `true`, and as `true` does where they differ.

use crate::{
    data::{Column, Data, Name},
    model::{Chart, Domain, ScaleType, Sort},
    transform::Op,
    vega_lite::{encoding, encoding::Def, sort, sort::DomainSort},
};
use gpui::SharedString;
use vega::statistics::Random;

/// A layer as Vega-Lite reads it.
pub(crate) struct Layered {
    pub(crate) chart: Chart,
    pub(crate) defs: Vec<Def>,
    /// Its rows before the aggregate.
    pub(crate) raw: Data,
    /// Each def's sort by an aggregate.
    pub(crate) sorts: Vec<Option<DomainSort>>,
}

/// One layer's part in a shared domain.
struct Part {
    layer: usize,
    /// The field its channel reads after the aggregate.
    field: SharedString,
    /// That field before the aggregate.
    key: SharedString,
    sort: Option<DomainSort>,
}

/// Gives each discrete channel two or more `layers` encode the values of
/// the domain Vega-Lite merges for them, in its order, unless one of them
/// lists its domain or has no scale.
pub(crate) fn share(layers: &mut [Layered], random: &mut Random) {
    let mut names: Vec<String> = Vec::new();
    for layer in layers.iter() {
        for def in &layer.defs {
            if !names.contains(&def.channel) {
                names.push(def.channel.clone());
            }
        }
    }
    for name in names {
        let Some(parts) = parts(layers, &name) else {
            continue;
        };
        let Some(values) = merge(layers, &parts, random) else {
            continue;
        };
        for part in &parts {
            let encoding = &mut layers[part.layer].chart.layers[0].encoding;
            if let Some(channel) = encoding::scaled_mut(encoding, &name) {
                channel.scale.domain = Some(Domain::Values(values.clone()));
            }
        }
    }
}

/// Each layer's part in the domain of channel `name`. `None` where fewer
/// than two layers encode a field on it discretely, or one lists its
/// domain or has no scale.
fn parts(layers: &mut [Layered], name: &str) -> Option<Vec<Part>> {
    let mut parts = Vec::new();
    for (index, layer) in layers.iter_mut().enumerate() {
        let Some(at) = layer.defs.iter().position(|def| def.channel == name) else {
            continue;
        };
        let def = &layer.defs[at];
        let encoding = &mut layer.chart.layers[0].encoding;
        let Some(channel) = encoding::scaled_mut(encoding, name) else {
            continue;
        };
        if !channel.kind.is_discrete() {
            continue;
        }
        if matches!(channel.scale.domain, Some(Domain::Values(_)))
            || channel.scale.kind == Some(ScaleType::Identity)
        {
            return None;
        }
        let field = channel.field()?.clone();
        let sort = match &layer.sorts[at] {
            Some(sort) => Some(sort.clone()),
            None => match &channel.sort {
                Sort::Data => None,
                Sort::Ascending => Some(DomainSort::Ascending),
                Sort::Descending => Some(DomainSort::By {
                    op: Op::Min,
                    field: Some(field.clone()),
                    descending: true,
                }),
                Sort::Explicit(listed) => Some(DomainSort::Listed(listed.clone())),
            },
        };
        parts.push(Part {
            layer: index,
            key: sort::key(def)?,
            field,
            sort,
        });
    }
    (parts.len() >= 2).then_some(parts)
}

/// The values of the domain `parts` share, in Vega's order. `None` where it
/// has no sort: its values as they come.
fn merge(layers: &[Layered], parts: &[Part], random: &mut Random) -> Option<Vec<SharedString>> {
    let column = |part: &Part| layers[part.layer].chart.layers[0].data.column(&part.field);
    let first = &parts[0];
    let one = parts
        .iter()
        .all(|part| part.field == first.field && same(column(part), column(first)));
    let mut sorts: Vec<&DomainSort> = Vec::new();
    for sort in parts.iter().filter_map(|part| part.sort.as_ref()) {
        if !sorts.contains(&sort) {
            sorts.push(sort);
        }
    }
    let sort = match one {
        // One domain: its one sort, or the one by an op other than `min`
        // among sorts all by an op.
        true => match sorts.len() {
            0 => return None,
            1 => sorts[0].clone(),
            _ => {
                let by_op = sorts.iter().all(|sort| **sort != DomainSort::Ascending);
                let others: Vec<&DomainSort> = sorts
                    .iter()
                    .copied()
                    .filter(|sort| matches!(sort, DomainSort::By { op, .. } if *op != Op::Min))
                    .collect();
                match (by_op, others.as_slice()) {
                    (true, [sort]) => (*sort).clone(),
                    _ => DomainSort::Ascending,
                }
            }
        },
        // A union: a sort by `count`, `min` or `max`, any other as `true`.
        false => {
            let mut kept: Vec<DomainSort> = Vec::new();
            for sort in sorts {
                let sort = match sort {
                    DomainSort::By { op, .. } if !matches!(op, Op::Count | Op::Min | Op::Max) => {
                        DomainSort::Ascending
                    }
                    sort => sort.clone(),
                };
                if !kept.contains(&sort) {
                    kept.push(sort);
                }
            }
            match kept.len() {
                0 => return None,
                1 => kept.remove(0),
                _ => DomainSort::Ascending,
            }
        }
    };
    let parts = match one {
        true => &parts[..1],
        false => parts,
    };
    // Every part's values: of its rows after the aggregate, or before it
    // for a sort by an op of another field, with that field's.
    let key: SharedString = "key".into();
    let value: SharedString = "value".into();
    let (sort, measured) = match sort {
        DomainSort::By {
            op,
            field,
            descending,
        } if op == Op::Count || field.as_ref().is_some_and(|f| *f != parts[0].key) => {
            let sort = DomainSort::By {
                op,
                field: field.as_ref().map(|_| value.clone()),
                descending,
            };
            (sort, field.filter(|_| op != Op::Count))
        }
        DomainSort::By { descending, .. } => (
            DomainSort::By {
                op: Op::Min,
                field: Some(key.clone()),
                descending,
            },
            None,
        ),
        sort => (sort, None),
    };
    let before = matches!(sort, DomainSort::By { ref field, .. } if field.as_ref() != Some(&key));
    let mut columns: Vec<Column> = Vec::new();
    let mut values: Vec<f64> = Vec::new();
    for part in parts {
        let layer = &layers[part.layer];
        let (data, field) = match before {
            true => (&layer.raw, &part.key),
            false => (&layer.chart.layers[0].data, &part.field),
        };
        columns.push(data.column(field)?.clone());
        if let Some(measured) = &measured {
            values.extend(data.numbers(measured));
        }
    }
    let union = match columns
        .iter()
        .all(|column| matches!(column, Column::Number(_)))
    {
        true => Data::new().number(
            key.clone(),
            columns
                .iter()
                .flat_map(|column| column.numbers().expect("numbers").to_vec())
                .collect::<Vec<_>>(),
        ),
        false => Data::new().with(
            key.clone(),
            Column::Text(
                columns
                    .iter()
                    .flat_map(|column| (0..column.len()).map(|row| Name::from(&column.value(row))))
                    .collect(),
            ),
        ),
    };
    let union = match measured {
        Some(_) => union.number(value, values),
        None => union,
    };
    let column = union.column(&key)?;
    // Each value's label and number, and whether it is null or undefined,
    // which comes first in ascending order and last in descending order, in
    // the order met.
    let mut distinct: Vec<(SharedString, f64, bool)> = Vec::new();
    let numeric = matches!(column, Column::Number(_));
    for (row, number) in union.numbers(&key).into_iter().enumerate() {
        let label = column.label(row);
        if !distinct.iter().any(|(other, ..)| *other == label) {
            distinct.push((label, number, column.value(row).is_nullish()));
        }
    }
    let natural = |descending: bool| {
        let mut sorted = distinct.clone();
        let ascending = |(a, x, a_null): &(SharedString, f64, bool),
                         (b, y, b_null): &(SharedString, f64, bool)| {
            b_null.cmp(a_null).then_with(|| match numeric {
                true => x.total_cmp(y),
                false => a.cmp(b),
            })
        };
        match descending {
            true => sorted.sort_by(|a, b| ascending(b, a)),
            false => sorted.sort_by(ascending),
        }
        sorted.into_iter().map(|(label, ..)| label).collect()
    };
    Some(match sort.order(&union, &key, random) {
        Sort::Ascending => natural(false),
        Sort::Descending => natural(true),
        Sort::Data => distinct.into_iter().map(|(label, ..)| label).collect(),
        Sort::Explicit(listed) => {
            let mut ranked = distinct;
            ranked.sort_by_key(|(label, ..)| {
                listed
                    .iter()
                    .position(|l| l == label)
                    .unwrap_or(listed.len())
            });
            ranked.into_iter().map(|(label, ..)| label).collect()
        }
    })
}

/// Whether two columns hold the same values.
fn same(a: Option<&Column>, b: Option<&Column>) -> bool {
    match (a, b) {
        (Some(Column::Number(a)), Some(Column::Number(b))) => {
            a.len() == b.len()
                && a.iter()
                    .zip(b.iter())
                    .all(|(x, y)| x.to_bits() == y.to_bits())
        }
        (Some(Column::Text(a)), Some(Column::Text(b))) => {
            a.codes.len() == b.codes.len() && (0..a.codes.len()).all(|row| a.get(row) == b.get(row))
        }
        _ => false,
    }
}
