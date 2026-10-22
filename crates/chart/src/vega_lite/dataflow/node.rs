//! A node of Vega-Lite's dataflow: what one step does to the rows passing
//! through it, and what Vega-Lite's optimizer reads of it.

use crate::{
    data::Data,
    expr::{self, Expr, Row},
    transform::{Aggregate, Bin, Bins, Measure, Op, Part, Random, TimeUnit, Transform},
    value::Value,
    vega_lite::{
        encoding::Def,
        field::{self, Derive},
    },
};
use d3::{
    format::NumberFormat,
    time::{Interval, Unit, date::LocalZone},
};
use gpui::SharedString;
use std::str::FromStr;

/// Vega-Lite's dash between a bin's edges in its text range.
const DASH: &str = " – ";

/// One op of a view's transforms as Vega-Lite normalizes them.
#[derive(Clone, Debug)]
pub(crate) enum Step {
    Transform(Transform),
    /// Every row, each with its group's measures.
    JoinAggregate(Aggregate),
    /// The rows the formula holds for.
    Filter(Formula),
    /// Every row, with the formula's value as a number under the name.
    Calculate(Formula, SharedString),
}

/// A Vega expression over a row.
#[derive(Clone, Debug)]
pub(crate) struct Formula {
    source: String,
    expr: Expr,
    /// Fields an aggregate wrote, whose missing values are `undefined`
    /// where a row's are `null`.
    pub(crate) aggregated: Vec<SharedString>,
}

impl FromStr for Formula {
    type Err = expr::Error;

    fn from_str(source: &str) -> Result<Self, expr::Error> {
        Ok(Self {
            source: source.to_string(),
            expr: source.parse()?,
            aggregated: Vec::new(),
        })
    }
}

/// Formulas are alike where their text is.
impl PartialEq for Formula {
    fn eq(&self, other: &Self) -> bool {
        self.source == other.source
    }
}

impl Formula {
    /// Its value on each row of `data`; `undefined` where JavaScript throws.
    /// `NaN` where an aggregated field is missing, as JavaScript's
    /// arithmetic and comparisons with `undefined` are in the formulas the
    /// composite marks write.
    fn values(&self, data: &Data) -> Vec<Value> {
        let aggregated: Vec<Vec<f64>> = self
            .aggregated
            .iter()
            .map(|field| data.numbers(field))
            .collect();
        (0..data.len())
            .map(|index| {
                if aggregated.iter().any(|values| values[index].is_nan()) {
                    return Value::Number(f64::NAN);
                }
                let row = Row {
                    data,
                    index,
                    params: &[],
                };
                self.expr.eval(&row).unwrap_or_default()
            })
            .collect()
    }
}

/// One bin a bin node computes, and whether it writes each bin's edges as
/// text, `<start>_range`.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Binning {
    pub(crate) bin: Bin,
    pub(crate) range: bool,
}

/// One formula a time unit node computes.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Truncation {
    Unit(TimeUnit),
    /// Where each step from `start` ends, for a field that holds times
    /// truncated to `units` already.
    End {
        start: SharedString,
        end: SharedString,
        units: Vec<Part>,
        step: f64,
    },
    /// Where a bar or rect spans each step from `start` to `end`, centred at
    /// `band` of it: `<start>_offsetted_rect_start` and `_end`.
    Rect {
        start: SharedString,
        end: SharedString,
        units: Vec<Part>,
        step: f64,
        band: f64,
    },
}

impl Truncation {
    /// The field it writes first.
    fn output(&self) -> SharedString {
        match self {
            Self::Unit(unit) => unit.output[0].clone(),
            Self::End { end, .. } => end.clone(),
            Self::Rect { start, .. } => format!("{start}_offsetted_rect_start").into(),
        }
    }
}

/// What a node does.
#[derive(Clone, Debug)]
pub(crate) enum Node {
    /// The rows of the source of this index.
    Source(usize),
    Bin(Vec<Binning>),
    TimeUnit(Vec<Truncation>),
    Aggregate(Aggregate),
    JoinAggregate(Aggregate),
    Filter(Formula),
    Calculate(Formula, SharedString),
    /// Density, loess, quantile or regression.
    Transform(Transform),
    /// Where a unit's marks or scales read the rows: they pass as they are.
    Output,
}

impl Node {
    /// The node a step makes. `None` for an aggregate of nothing, which
    /// Vega-Lite drops.
    pub(crate) fn of(step: &Step) -> Option<Self> {
        Some(match step {
            Step::Transform(Transform::Aggregate(op)) => Self::aggregate(op.clone())?,
            Step::Transform(Transform::Bin(bin)) => Self::Bin(vec![Binning {
                bin: bin.clone(),
                range: false,
            }]),
            Step::Transform(Transform::TimeUnit(unit)) => {
                Self::TimeUnit(vec![Truncation::Unit(unit.clone())])
            }
            Step::Transform(op) => Self::Transform(op.clone()),
            Step::JoinAggregate(op) => Self::JoinAggregate(op.clone()),
            Step::Filter(formula) => Self::Filter(formula.clone()),
            Step::Calculate(formula, output) => Self::Calculate(formula.clone(), output.clone()),
        })
    }

    /// An aggregate node, its measures as Vega-Lite keeps them: by field in
    /// the order met, each field's ops in the order met, each op's names.
    fn aggregate(op: Aggregate) -> Option<Self> {
        if op.groupby.is_empty() && op.measures.is_empty() {
            return None;
        }
        let mut out = Aggregate {
            groupby: op.groupby,
            measures: Vec::new(),
        };
        merge_measures(&mut out.measures, op.measures);
        Some(Self::Aggregate(out))
    }

    /// The bins `defs` derive, each once.
    pub(crate) fn bins(defs: &[Def]) -> Option<Self> {
        let mut bins: Vec<Binning> = Vec::new();
        for def in defs {
            let Derive::Bin(bin) = &def.derive else {
                continue;
            };
            match bins.iter_mut().find(|other| other.bin.output == bin.output) {
                Some(other) => other.range |= def.range,
                None => bins.push(Binning {
                    bin: bin.clone(),
                    range: def.range,
                }),
            }
        }
        (!bins.is_empty()).then_some(Self::Bin(bins))
    }

    /// The time units `defs` derive, each once, then where their steps end
    /// and where bars and rects span them.
    pub(crate) fn time_units(defs: &[Def]) -> Option<Self> {
        let mut formulas: Vec<Truncation> = Vec::new();
        for def in defs {
            if let Derive::TimeUnit(unit) = &def.derive {
                let unit = Truncation::Unit(unit.clone());
                if !formulas.contains(&unit) {
                    formulas.push(unit);
                }
            }
        }
        for def in defs {
            let (Some([start, end]), Some((units, step))) = (def.edges(), def.derive.units())
            else {
                continue;
            };
            if def.end && matches!(def.derive, Derive::BinnedTimeUnit { .. }) {
                formulas.push(Truncation::End {
                    start: start.clone(),
                    end: end.clone(),
                    units: units.to_vec(),
                    step,
                });
            }
            if let Some(band) = def.rect_band {
                formulas.push(Truncation::Rect {
                    start,
                    end,
                    units: units.to_vec(),
                    step,
                    band,
                });
            }
        }
        (!formulas.is_empty()).then_some(Self::TimeUnit(formulas))
    }

    /// Vega-Lite's aggregate of an encoding: every aggregated field's
    /// measure, grouped by every field not aggregated. `None` where no field
    /// is aggregated.
    pub(crate) fn encoding_aggregate(defs: &[Def]) -> Option<Self> {
        if !defs
            .iter()
            .any(|def| matches!(def.derive, Derive::Aggregate { .. }))
        {
            return None;
        }
        let mut groupby: Vec<SharedString> = Vec::new();
        let mut measures: Vec<Measure> = Vec::new();
        for def in defs {
            match &def.derive {
                Derive::Aggregate {
                    op,
                    measured,
                    param,
                    output,
                } => {
                    let measure = Measure {
                        op: *op,
                        field: measured.clone().filter(|_| *op != Op::Count),
                        param: *param,
                        output: output.clone(),
                    };
                    // One name per op of a field: the encoding's own.
                    replace_measure(&mut measures, measure);
                    if def.unaggregated
                        && let Some(field) = &def.field
                    {
                        for op in [Op::Min, Op::Max] {
                            replace_measure(
                                &mut measures,
                                Measure {
                                    op,
                                    field: Some(field.clone()),
                                    param: None,
                                    output: field::output(op, field),
                                },
                            );
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
        Self::aggregate(Aggregate { groupby, measures })
    }

    /// Whether Vega-Lite's optimizer takes the two for one: its hash of
    /// each is the same.
    pub(crate) fn same(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Source(a), Self::Source(b)) => a == b,
            (Self::Bin(a), Self::Bin(b)) => same_set(a, b),
            (Self::TimeUnit(a), Self::TimeUnit(b)) => same_set(a, b),
            (Self::Aggregate(a), Self::Aggregate(b))
            | (Self::JoinAggregate(a), Self::JoinAggregate(b)) => {
                a.groupby == b.groupby && sorted(&a.measures) == sorted(&b.measures)
            }
            (Self::Filter(a), Self::Filter(b)) => a == b,
            (Self::Calculate(a, x), Self::Calculate(b, y)) => a == b && x == y,
            (Self::Transform(a), Self::Transform(b)) => a == b,
            _ => false,
        }
    }

    /// The fields it writes.
    pub(crate) fn produces(&self) -> Vec<SharedString> {
        match self {
            Self::Bin(bins) => bins
                .iter()
                .flat_map(|binning| binning.bin.output.clone())
                .collect(),
            Self::TimeUnit(formulas) => formulas.iter().map(Truncation::output).collect(),
            Self::Aggregate(op) | Self::JoinAggregate(op) => op
                .measures
                .iter()
                .map(|measure| measure.output.clone())
                .collect(),
            Self::Calculate(_, output) => vec![output.clone()],
            Self::Transform(op) => match op {
                Transform::Density(op) => op.output.to_vec(),
                Transform::Loess(op) => op.output.to_vec(),
                Transform::Regression(op) => op.output.to_vec(),
                Transform::Quantile(op) => op.output.to_vec(),
                Transform::Aggregate(_) | Transform::Bin(_) | Transform::TimeUnit(_) => Vec::new(),
            },
            Self::Source(_) | Self::Filter(_) | Self::Output => Vec::new(),
        }
    }

    /// The fields a bin node reads.
    pub(crate) fn binned(&self) -> Vec<SharedString> {
        match self {
            Self::Bin(bins) => bins
                .iter()
                .map(|binning| binning.bin.field.clone())
                .collect(),
            _ => Vec::new(),
        }
    }

    /// Takes in a node Vega-Lite's optimizer merges into this one: a bin's
    /// or time unit's formulas this lacks, an aggregate's measures.
    pub(crate) fn absorb(&mut self, other: &Self) {
        match (self, other) {
            (Self::Bin(bins), Self::Bin(others)) => {
                for binning in others {
                    match bins.iter_mut().find(|b| b.bin == binning.bin) {
                        Some(bin) => bin.range |= binning.range,
                        None => bins.push(binning.clone()),
                    }
                }
            }
            (Self::TimeUnit(formulas), Self::TimeUnit(others)) => {
                for formula in others {
                    if !formulas.contains(formula) {
                        formulas.push(formula.clone());
                    }
                }
            }
            (Self::Aggregate(op), Self::Aggregate(other)) => {
                merge_measures(&mut op.measures, other.measures.clone())
            }
            _ => {}
        }
    }

    /// The rows it makes of `data`, and each bin's edges it computes by the
    /// field of the bin's starts.
    pub(crate) fn apply(
        &self,
        data: &Data,
        random: &mut Random,
    ) -> (Data, Vec<(SharedString, Bins)>) {
        let data = match self {
            Self::Source(_) | Self::Output => data.clone(),
            Self::Bin(bins) => {
                let mut data = data.clone();
                let mut edges = Vec::new();
                for binning in bins {
                    edges.push((binning.bin.output[0].clone(), binning.bin.bins(&data)));
                    data = binning.bin.apply(&data);
                }
                for binning in bins.iter().filter(|binning| binning.range) {
                    data = range(data, &binning.bin.output);
                }
                return (data, edges);
            }
            Self::TimeUnit(formulas) => {
                let mut data = data.clone();
                for formula in formulas {
                    if let Truncation::Unit(unit) = formula {
                        data = unit.apply(&data);
                    }
                }
                for formula in formulas {
                    data = match formula {
                        Truncation::Unit(_) => data,
                        Truncation::End {
                            start,
                            end,
                            units,
                            step,
                        } => {
                            let ends = offset(&data.numbers(start), units, *step, 1.0);
                            data.times(end.clone(), ends)
                        }
                        // Vega-Lite's `interpolateExpr` between the step
                        // before and the step's start, and between its start
                        // and end.
                        Truncation::Rect {
                            start,
                            end,
                            units,
                            step,
                            band,
                        } => {
                            let f = band + 0.5;
                            let starts = data.numbers(start);
                            let before = offset(&starts, units, *step, -1.0);
                            let ends = data.numbers(end);
                            let lerp = |a: &[f64], b: &[f64]| -> Vec<f64> {
                                a.iter()
                                    .zip(b)
                                    .map(|(a, b)| (1.0 - f) * a + f * b)
                                    .collect()
                            };
                            data.times(
                                format!("{start}_offsetted_rect_start"),
                                lerp(&before, &starts),
                            )
                            .times(format!("{start}_offsetted_rect_end"), lerp(&starts, &ends))
                        }
                    };
                }
                data
            }
            Self::Aggregate(op) => op.apply(data, random),
            Self::JoinAggregate(op) => op.join(data, random),
            Self::Filter(formula) => {
                let keep: Vec<usize> = formula
                    .values(data)
                    .iter()
                    .enumerate()
                    .filter(|(_, value)| value.truthy())
                    .map(|(row, _)| row)
                    .collect();
                let mut out = Data::new();
                for (field, _) in data.fields() {
                    out = out.adopt(data, field.clone(), data.take(field, &keep));
                }
                out
            }
            Self::Calculate(formula, output) => {
                let values: Vec<f64> = formula
                    .values(data)
                    .iter()
                    .map(|value| match value {
                        Value::Undefined | Value::Null => f64::NAN,
                        value => value.to_number(),
                    })
                    .collect();
                data.clone().number(output.clone(), values)
            }
            Self::Transform(op) => op.apply(data, random),
        };
        (data, Vec::new())
    }
}

/// `data` with the text range of the bins `output` names, as
/// `<start>_range`.
fn range(data: Data, output: &[SharedString; 2]) -> Data {
    let [start, end] = output;
    let starts = data.numbers(start);
    let ends = data.numbers(end);
    let format: NumberFormat = "".parse().expect("an empty specifier");
    let ranges: Vec<String> = starts
        .iter()
        .zip(&ends)
        .map(|(&start, &end)| match start.is_finite() {
            true => format!("{}{DASH}{}", format.format(start), format.format(end)),
            false => "null".into(),
        })
        .collect();
    data.text(format!("{start}_range"), ranges)
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

/// A count measures no field, whatever it names.
fn field_of(measure: &Measure) -> Option<&SharedString> {
    measure.field.as_ref().filter(|_| measure.op != Op::Count)
}

/// Vega-Lite's `mergeMeasures`: each of `others` among its field's ops, a
/// name new to its op after the op's others, an op new to its field after
/// the field's others, a field new to `measures` last; an op takes the last
/// weight given.
fn merge_measures(measures: &mut Vec<Measure>, others: Vec<Measure>) {
    for measure in others {
        let same_op = |m: &Measure| field_of(m) == field_of(&measure) && m.op == measure.op;
        if let Some(param) = measure.param {
            for m in measures.iter_mut().filter(|m| same_op(m)) {
                m.param = Some(param);
            }
        }
        if measures
            .iter()
            .any(|m| same_op(m) && m.output == measure.output)
        {
            continue;
        }
        let param = measures
            .iter()
            .find(|m| same_op(m))
            .map_or(measure.param, |m| m.param);
        let at = measures
            .iter()
            .rposition(same_op)
            .or_else(|| {
                measures
                    .iter()
                    .rposition(|m| field_of(m) == field_of(&measure))
            })
            .map_or(measures.len(), |at| at + 1);
        measures.insert(at, Measure { param, ..measure });
    }
}

/// An encoding's measure, over one it already has of the same op and field.
fn replace_measure(measures: &mut Vec<Measure>, measure: Measure) {
    match measures
        .iter_mut()
        .find(|m| field_of(m) == field_of(&measure) && m.op == measure.op)
    {
        Some(met) => met.param = measure.param.or(met.param),
        None => measures.push(measure),
    }
}

/// One op of a field's measures as Vega-Lite hashes it.
#[derive(PartialEq)]
struct Hashed {
    field: Option<String>,
    op: String,
    param: Option<u64>,
    names: Vec<SharedString>,
}

/// Measures as Vega-Lite hashes them: by field and op, each op's names in
/// order.
fn sorted(measures: &[Measure]) -> Vec<Hashed> {
    let mut keys: Vec<Hashed> = Vec::new();
    for measure in measures {
        let field = field_of(measure).map(|field| field.to_string());
        let op = measure.op.to_string();
        match keys
            .iter_mut()
            .find(|key| key.field == field && key.op == op)
        {
            Some(key) => key.names.push(measure.output.clone()),
            None => keys.push(Hashed {
                field,
                op,
                param: measure.param.map(f64::to_bits),
                names: vec![measure.output.clone()],
            }),
        }
    }
    keys.sort_by(|a, b| (&a.field, &a.op).cmp(&(&b.field, &b.op)));
    keys
}

/// Whether two lists hold the same items, in any order.
fn same_set<T: PartialEq>(a: &[T], b: &[T]) -> bool {
    a.len() == b.len() && a.iter().all(|item| b.contains(item))
}
