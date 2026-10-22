//! What Vega-Lite derives from a view's encoding before its marks, in the
//! order its dataflow runs: the encoding's bins, before a layer's own
//! `transform` list and after any other, each with its text range where a
//! channel reads it; then the time units; then an aggregate over the groups
//! of every field not aggregated; then where in each bin or step a mark
//! that does not span it stands. A discrete channel sorted by an aggregate
//! is ordered over the rows before the aggregate.

use crate::{
    data::{Column, Data},
    model::{Mark, Sort},
    scale,
    transform::{Aggregate, Bin, Bins, Measure, Op, Part, Random, TimeUnit, Transform},
    vega_lite::{
        encoding::{Def, SortBy},
        field::{self, Derive},
    },
};
use gpui::SharedString;
use js::{
    date::LocalZone,
    format::NumberFormat,
    time::{Interval, Unit},
};

/// Vega-Lite's dash between a bin's edges in its text range.
const DASH: &str = " – ";
/// Channels that split a stack.
const STACK_BY: [&str; 9] = [
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

/// A view's data through `pipeline` and what its encoding's `defs` derive,
/// the bins before the op at `bins_at`, and the order each sorted channel's
/// values take, by def, every op drawing from `random`. `None` for a sort
/// Vega cannot run.
pub(crate) fn run(
    source: Data,
    pipeline: &[Transform],
    bins_at: usize,
    defs: &[Def],
    mark: Mark,
    random: &mut Random,
) -> Option<Flow> {
    let mut bins: Vec<&Bin> = Vec::new();
    let mut units: Vec<&TimeUnit> = Vec::new();
    for def in defs {
        match &def.derive {
            Derive::Bin(bin) if !bins.iter().any(|other| other.output == bin.output) => {
                bins.push(bin)
            }
            Derive::TimeUnit(unit) if !units.iter().any(|other| other.output == unit.output) => {
                units.push(unit)
            }
            _ => {}
        }
    }
    let units: Vec<Transform> = units
        .into_iter()
        .cloned()
        .map(Transform::TimeUnit)
        .collect();
    let (before, after) = pipeline.split_at(bins_at.min(pipeline.len()));
    let mut data = source.transform(before, random);
    let mut edges = Vec::new();
    for bin in &bins {
        edges.push((bin.output[0].clone(), bin.bins(&data)));
        data = bin.apply(&data);
    }
    let data = ranges(data, defs).transform(after, random);
    let mut raw = data.transform(&units, random);
    for def in defs {
        let (Some([start, end]), Some((units, step))) = (def.edges(), def.derive.units()) else {
            continue;
        };
        if def.end && matches!(def.derive, Derive::BinnedTimeUnit { .. }) {
            let ends = offset(&raw.numbers(&start), units, step, 1.0);
            raw = raw.times(end.clone(), ends);
        }
        // Vega-Lite's `interpolateExpr` between the step before and the
        // step's start, and between its start and end.
        if let Some(p) = def.rect_band {
            let f = p + 0.5;
            let starts = raw.numbers(&start);
            let before = offset(&starts, units, step, -1.0);
            let ends = raw.numbers(&end);
            let lerp = |a: &[f64], b: &[f64]| -> Vec<f64> {
                a.iter()
                    .zip(b)
                    .map(|(a, b)| (1.0 - f) * a + f * b)
                    .collect()
            };
            raw = raw
                .times(
                    format!("{start}_offsetted_rect_start"),
                    lerp(&before, &starts),
                )
                .times(format!("{start}_offsetted_rect_end"), lerp(&starts, &ends));
        }
    }

    let stacked = matches!(mark, Mark::Bar | Mark::Area)
        && defs.iter().any(|def| {
            STACK_BY.contains(&def.channel.as_str())
                && def.field.is_some()
                && !matches!(def.derive, Derive::Aggregate { .. })
        });
    let mut sorts = Vec::with_capacity(defs.len());
    let mut orders = Vec::with_capacity(defs.len());
    for def in defs {
        let Some(sort) = &def.sort else {
            sorts.push(None);
            orders.push(None);
            continue;
        };
        let sort = DomainSort::of(def, sort, stacked, defs)?;
        orders.push(Some(sort.order(&raw, &key(def)?, random)));
        sorts.push(Some(sort));
    }
    let data = match aggregate(defs) {
        Some(aggregate) => aggregate.apply(&raw, random),
        None => raw.clone(),
    };
    let data = bands(data, defs);
    Some(Flow {
        data,
        raw,
        bins: edges,
        sorts,
        orders,
    })
}

/// A view's data as its marks draw it.
pub(crate) struct Flow {
    pub(crate) data: Data,
    /// Each encoded bin's edges, by the field of its starts.
    pub(crate) bins: Vec<(SharedString, Bins)>,
    /// The rows before the aggregate.
    pub(crate) raw: Data,
    /// Each def's sort by an aggregate.
    pub(crate) sorts: Vec<Option<DomainSort>>,
    /// The order of each def's values, for a def sorted by an aggregate.
    pub(crate) orders: Vec<Option<Sort>>,
}

/// The text ranges of the bins `defs` read them from.
fn ranges(mut data: Data, defs: &[Def]) -> Data {
    for def in defs {
        let Derive::Bin(bin) = &def.derive else {
            continue;
        };
        let [start, end] = &bin.output;
        let starts = data.numbers(start);
        let ends = data.numbers(end);
        if def.range {
            let format: NumberFormat = "".parse().expect("an empty specifier");
            let ranges: Vec<String> = starts
                .iter()
                .zip(&ends)
                .map(|(&start, &end)| match start.is_finite() {
                    true => format!("{}{DASH}{}", format.format(start), format.format(end)),
                    false => "null".into(),
                })
                .collect();
            data = data.text(format!("{start}_range"), ranges);
        }
    }
    data
}

/// Where marks stand in the bins and steps `defs` place them in.
fn bands(mut data: Data, defs: &[Def]) -> Data {
    for def in defs {
        let (Some(p), Some([start, end])) = (def.band, def.edges()) else {
            continue;
        };
        let places: Vec<f64> = data
            .numbers(&start)
            .iter()
            .zip(data.numbers(&end))
            .map(|(start, end)| (1.0 - p) * start + p * end)
            .collect();
        data = data.number(field::band(&start, p), places);
    }
    data
}

/// `times` moved `n` steps of the least of `units`, as Vega-Lite's
/// `timeOffset` of that unit's date part moves them.
fn offset(times: &[f64], units: &[Part], step: f64, n: f64) -> Vec<f64> {
    let zone = LocalZone::utc();
    let (unit, step) = match units.last() {
        Some(Part::Year) => (Unit::Year, step),
        Some(Part::Quarter) => (Unit::Month, 3.0 * step),
        Some(Part::Month) => (Unit::Month, step),
        Some(Part::Week) => (Unit::Day, 7.0 * step),
        Some(Part::Hours) => (Unit::Hour, step),
        Some(Part::Minutes) => (Unit::Minute, step),
        Some(Part::Seconds) => (Unit::Second, step),
        Some(Part::Milliseconds) => (Unit::Millisecond, step),
        _ => (Unit::Day, step),
    };
    let interval = Interval::new(unit, 0.0);
    times
        .iter()
        .map(|&time| {
            let mut date = zone.date(time);
            interval.offset(&mut date, (n * step).floor());
            date.time()
        })
        .collect()
}

/// Vega-Lite's aggregate of an encoding: every aggregated field's measure,
/// grouped by every field not aggregated. `None` where no field is
/// aggregated.
fn aggregate(defs: &[Def]) -> Option<Aggregate> {
    if !defs
        .iter()
        .any(|def| matches!(def.derive, Derive::Aggregate { .. }))
    {
        return None;
    }
    let mut groupby: Vec<SharedString> = Vec::new();
    // Measures by field, `None` for a count, ops in the order met.
    let mut measured: Vec<(Option<SharedString>, Vec<Measure>)> = Vec::new();
    let mut measure = |measure: Measure| {
        let slot = match measured.iter_mut().find(|(f, _)| *f == measure.field) {
            Some((_, ops)) => ops,
            None => {
                measured.push((measure.field.clone(), Vec::new()));
                &mut measured.last_mut().expect("just pushed").1
            }
        };
        match slot.iter_mut().find(|m| m.op == measure.op) {
            Some(met) => met.param = measure.param.or(met.param),
            None => slot.push(measure),
        }
    };
    for def in defs {
        match &def.derive {
            Derive::Aggregate {
                op,
                measured: field,
                param,
                output,
            } => {
                measure(Measure {
                    op: *op,
                    field: field.clone().filter(|_| *op != Op::Count),
                    param: *param,
                    output: output.clone(),
                });
                if def.unaggregated
                    && let Some(field) = &def.field
                {
                    for op in [Op::Min, Op::Max] {
                        measure(Measure {
                            op,
                            field: Some(field.clone()),
                            param: None,
                            output: field::output(op, field),
                        });
                    }
                }
            }
            _ => {
                let Some(field) = &def.field else {
                    continue;
                };
                let names: Vec<SharedString> = match def.edges().filter(|_| def.end) {
                    Some([start, end]) => {
                        let mut names = vec![start.clone(), end];
                        if def.rect_band.is_some() {
                            names.push(format!("{start}_offsetted_rect_start").into());
                            names.push(format!("{start}_offsetted_rect_end").into());
                        }
                        if def.range {
                            names.push(format!("{start}_range").into());
                        }
                        names
                    }
                    None => def.derive.name(Some(field)).into_iter().collect(),
                };
                for name in names {
                    if !groupby.contains(&name) {
                        groupby.push(name);
                    }
                }
            }
        }
    }
    let measures: Vec<Measure> = measured.into_iter().flat_map(|(_, ops)| ops).collect();
    (!groupby.is_empty() || !measures.is_empty()).then_some(Aggregate { groupby, measures })
}

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
    fn of(def: &Def, sort: &SortBy, stacked: bool, defs: &[Def]) -> Option<Self> {
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
        let labels: Vec<SharedString> = match groups.column(key) {
            Some(Column::Text(text)) => {
                (0..groups.len()).map(|row| text.get(row).clone()).collect()
            }
            Some(Column::Number(numbers)) => {
                numbers.iter().map(|&v| scale::value(v).into()).collect()
            }
            None => return Sort::Explicit(Vec::new()),
        };
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
