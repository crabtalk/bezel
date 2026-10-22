//! Vega's sort of a discrete domain by an aggregate of each value's rows, as
//! Vega-Lite writes it.

use crate::{
    data::Data,
    model::Sort,
    transform::{Aggregate, Measure, Op},
    vega_lite::{
        encoding::{Def, SortBy},
        field::Derive,
    },
};
use gpui::SharedString;
use vega::statistics::Random;

/// Channels that split a stack.
pub(crate) const STACK_BY: [&str; 9] = [
    "color",
    "fill",
    "stroke",
    "opacity",
    "fillOpacity",
    "strokeOpacity",
    "strokeWidth",
    "strokeDash",
    "detail",
];

/// Vega's sort of a discrete domain, as Vega-Lite writes it.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum DomainSort {
    /// `true`: the values ascending.
    Ascending,
    /// The values in a listed order, any other after them.
    Listed(Vec<SharedString>),
    /// `op` of `field` over each value's rows; a count reads no field.
    By {
        op: Op,
        field: Option<SharedString>,
        descending: bool,
    },
}

impl DomainSort {
    /// Vega-Lite's sort of `def`'s domain by `sort`: an op it leaves out is
    /// `sum` for a stacked measure and `min` otherwise. `None` for a sort by
    /// no field with an op other than `count`.
    pub(crate) fn of(def: &Def, sort: &SortBy, stacked: bool, defs: &[Def]) -> Option<Self> {
        let dimension = |field: &SharedString| {
            defs.iter().any(|d| {
                d.field.as_ref() == Some(field)
                    && !matches!(d.derive, Derive::Aggregate { .. })
                    && (d.channel == def.channel || STACK_BY.contains(&d.channel.as_str()))
            })
        };
        let op = match sort.op {
            Some(op) => op,
            None if stacked && !sort.field.as_ref().is_some_and(dimension) => Op::Sum,
            None => Op::Min,
        };
        if op != Op::Count && sort.field.is_none() {
            return None;
        }
        Some(Self::By {
            op,
            field: sort.field.clone().filter(|_| op != Op::Count),
            descending: sort.descending,
        })
    }

    /// The order this puts the values of `key` in over `raw`: Vega's domain
    /// sort, ties in the order values first appear. A sort by `key` itself
    /// sorts its values.
    pub(crate) fn order(&self, raw: &Data, key: &SharedString, random: &mut Random) -> Sort {
        let (op, field, descending) = match self {
            Self::Ascending => return Sort::Ascending,
            Self::Listed(listed) => return Sort::Explicit(listed.clone()),
            Self::By {
                op,
                field,
                descending,
            } => (*op, field, *descending),
        };
        if field.as_ref() == Some(key) && op != Op::Count {
            return match descending {
                true => Sort::Descending,
                false => Sort::Ascending,
            };
        }
        let measure = Measure {
            op,
            field: field.clone(),
            param: None,
            output: "value".into(),
        };
        let groups = Aggregate {
            groupby: vec![key.clone()],
            measures: vec![measure],
        }
        .apply(raw, random);
        let values = groups.numbers("value");
        let mut rank: Vec<usize> = (0..groups.len()).collect();
        rank.sort_by(|&a, &b| {
            let ordering = js_compare(values[a], values[b]);
            match descending {
                true => ordering.reverse(),
                false => ordering,
            }
        });
        let Some(column) = groups.column(key) else {
            return Sort::Explicit(Vec::new());
        };
        let labels: Vec<SharedString> = (0..groups.len()).map(|row| column.label(row)).collect();
        Sort::Explicit(rank.into_iter().map(|row| labels[row].clone()).collect())
    }
}

/// The field `def`'s channel reads its values from, before an aggregate.
pub(crate) fn key(def: &Def) -> Option<SharedString> {
    let key = def.derive.name(def.field.as_ref())?;
    Some(match (&def.derive, def.range) {
        (Derive::Bin(bin), true) => format!("{}_range", bin.output[0]).into(),
        _ => key,
    })
}

/// vega-util's `ascending` for numbers: a missing value first.
fn js_compare(a: f64, b: f64) -> std::cmp::Ordering {
    use std::cmp::Ordering;
    match (a.is_nan(), b.is_nan()) {
        (true, true) => Ordering::Equal,
        (true, false) => Ordering::Less,
        (false, true) => Ordering::Greater,
        (false, false) => a.partial_cmp(&b).unwrap_or(Ordering::Equal),
    }
}

/// Whether Vega-Lite's scale of `def` sorts by an op over the rows before
/// the aggregate: any sort but a bin's text ascending by where each bin starts.
pub(crate) fn by_op(def: &Def) -> bool {
    match (&def.sort, &def.derive) {
        (None, _) => false,
        (Some(sort), Derive::Bin(bin)) if def.range => {
            !(sort.op == Some(Op::Min)
                && sort.field.as_ref() == Some(&bin.output[0])
                && !sort.descending)
        }
        _ => true,
    }
}
