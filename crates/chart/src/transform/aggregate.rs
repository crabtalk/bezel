//! Vega-Lite's `aggregate` transform, each op as Vega computes it.

use crate::{
    data::{Column, Data, Name, Text},
    transform::Groups,
};
use d3::{
    array,
    format::{number, number::Number as JsNumber},
    string,
};
use gpui::SharedString;
use std::{borrow::Cow, cmp::Ordering, collections::HashSet, fmt, str::FromStr};
use vega::{statistics::Random, value::Value};

/// Resamples per bootstrap confidence interval, and its alpha.
const SAMPLES: usize = 1000;
const ALPHA: f64 = 0.05;

/// Vega's aggregate ops, by their Vega names.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Op {
    /// Rows in the group, whatever the field holds.
    Count,
    /// The group's rows, each an object of every field, in an array.
    Values,
    Valid,
    Missing,
    Distinct,
    Sum,
    Product,
    Mean,
    Average,
    Variance,
    Variancep,
    Stdev,
    Stdevp,
    Stderr,
    Median,
    Q1,
    Q3,
    Ci0,
    Ci1,
    Min,
    Max,
    /// The row with the least value: every field of it, each as
    /// `<output>.<field>`.
    Argmin,
    Argmax,
    Exponential,
    Exponentialb,
}

const OPS: [(Op, &str); 25] = [
    (Op::Count, "count"),
    (Op::Values, "values"),
    (Op::Valid, "valid"),
    (Op::Missing, "missing"),
    (Op::Distinct, "distinct"),
    (Op::Sum, "sum"),
    (Op::Product, "product"),
    (Op::Mean, "mean"),
    (Op::Average, "average"),
    (Op::Variance, "variance"),
    (Op::Variancep, "variancep"),
    (Op::Stdev, "stdev"),
    (Op::Stdevp, "stdevp"),
    (Op::Stderr, "stderr"),
    (Op::Median, "median"),
    (Op::Q1, "q1"),
    (Op::Q3, "q3"),
    (Op::Ci0, "ci0"),
    (Op::Ci1, "ci1"),
    (Op::Min, "min"),
    (Op::Max, "max"),
    (Op::Argmin, "argmin"),
    (Op::Argmax, "argmax"),
    (Op::Exponential, "exponential"),
    (Op::Exponentialb, "exponentialb"),
];

impl FromStr for Op {
    type Err = ();

    fn from_str(name: &str) -> Result<Self, ()> {
        OPS.iter()
            .find(|(_, n)| *n == name)
            .map(|(op, _)| *op)
            .ok_or(())
    }
}

impl fmt::Display for Op {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let (_, name) = OPS
            .iter()
            .find(|(op, _)| op == self)
            .expect("every op is named");
        f.write_str(name)
    }
}

impl Op {
    /// Vega's order of computing a field's ops.
    fn index(self) -> i8 {
        match self {
            Op::Values => -1,
            Op::Average | Op::Variance | Op::Exponentialb => 1,
            Op::Variancep | Op::Stdev | Op::Stdevp | Op::Stderr => 2,
            Op::Distinct
            | Op::Ci0
            | Op::Ci1
            | Op::Median
            | Op::Q1
            | Op::Q3
            | Op::Argmin
            | Op::Argmax => 3,
            Op::Min | Op::Max => 4,
            _ => 0,
        }
    }
}

/// One aggregated value per group.
#[derive(Clone, Debug, PartialEq)]
pub struct Measure {
    pub op: Op,
    /// `None` only for `count` and `values`.
    pub field: Option<SharedString>,
    /// `exponential`'s weight, which `exponentialb` takes from an
    /// `exponential` of the same field. `None` computes `NaN`.
    pub param: Option<f64>,
    pub output: SharedString,
}

/// One row per group of `groupby`'s values, in the order groups first
/// appear: the group's values, then each measure.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Aggregate {
    pub groupby: Vec<SharedString>,
    pub measures: Vec<Measure>,
}

/// A field's value in a row as Vega's aggregate reads it.
#[derive(Clone, Copy)]
enum Cell {
    /// `null`, `undefined` or `""`.
    Missing,
    Valid(f64),
}

/// The running sums Vega keeps for one field of one group.
struct Running {
    valid: f64,
    missing: f64,
    sum: f64,
    product: f64,
    mean: f64,
    dev: f64,
    exp: f64,
    exponent: f64,
    /// The row of the least and greatest value, first met: text compared
    /// as JavaScript compares strings.
    least: Option<(usize, f64)>,
    most: Option<(usize, f64)>,
    first: Option<usize>,
}

/// Which field Vega's per-group tuple store last computed for, and what it
/// keeps from that: its caches are shared by every field of the group.
#[derive(Default)]
struct Store {
    get: Option<usize>,
    extent: bool,
    quartiles: Option<[f64; 3]>,
    ci: Option<[f64; 2]>,
}

impl Aggregate {
    /// `ci0` and `ci1` draw their resamples from `random`, group by group.
    pub fn apply(&self, data: &Data, random: &mut Random) -> Data {
        let groups = Groups::of(data, &self.groupby);
        let owners: Vec<usize> = groups.rows.iter().map(|rows| rows[0]).collect();
        let mut table = groups.keys(data, &owners);
        // Each field's measures, fields in the order first measured.
        let mut fields: Vec<(&SharedString, Vec<usize>)> = Vec::new();
        for (i, measure) in self.measures.iter().enumerate() {
            let Some(field) = measure.field.as_ref().filter(|_| measure.op != Op::Count) else {
                continue;
            };
            match fields.iter_mut().find(|(name, _)| *name == field) {
                Some((_, measures)) => measures.push(i),
                None => fields.push((field, vec![i])),
            }
        }
        for (_, measures) in &mut fields {
            measures.sort_by_key(|&i| self.measures[i].op.index());
        }
        let cells: Vec<(Vec<Cell>, Vec<u64>)> = fields
            .iter()
            .map(|(field, _)| (cells(data, field), data.keys(field)))
            .collect();
        let columns: Vec<Option<Cow<Column>>> =
            fields.iter().map(|(field, _)| data.field(field)).collect();

        let mut values = vec![vec![f64::NAN; groups.rows.len()]; self.measures.len()];
        let mut picked: Vec<Vec<Option<usize>>> = vec![Vec::new(); self.measures.len()];
        for (group, rows) in groups.rows.iter().enumerate() {
            for (i, measure) in self.measures.iter().enumerate() {
                if measure.op == Op::Count {
                    values[i][group] = rows.len() as f64;
                }
            }
            let mut store = Store::default();
            for (field, (((_, measures), (cells, keys)), column)) in
                fields.iter().zip(&cells).zip(&columns).enumerate()
            {
                let exponent = measures
                    .iter()
                    .map(|&i| &self.measures[i])
                    .rfind(|m| m.op == Op::Exponential)
                    .and_then(|m| m.param)
                    .unwrap_or(f64::NAN);
                let text = match column.as_deref() {
                    Some(Column::Text(text)) => Some(text),
                    _ => None,
                };
                let run = Running::of(rows, cells, text, exponent);
                for &i in measures {
                    match self.measures[i].op {
                        op @ (Op::Argmin | Op::Argmax) => {
                            picked[i].push(run.pick(op, &mut store, field))
                        }
                        Op::Values => {}
                        Op::Min if text.is_some() => picked[i].push(run.least.map(|(row, _)| row)),
                        Op::Max if text.is_some() => picked[i].push(run.most.map(|(row, _)| row)),
                        op => {
                            values[i][group] = run
                                .value(op, rows, (cells, keys), &mut store, field, random)
                                .unwrap_or(f64::NAN)
                        }
                    }
                }
            }
        }

        // Vega writes the counts, then each field's measures in the order
        // it computes them: of two measures of one name, the later. A
        // `values` of no field last.
        let counts = (0..self.measures.len()).filter(|&i| self.measures[i].op == Op::Count);
        let fieldless = (0..self.measures.len())
            .filter(|&i| self.measures[i].op == Op::Values && self.measures[i].field.is_none());
        let order: Vec<usize> = counts
            .chain(
                fields
                    .iter()
                    .flat_map(|(_, measures)| measures.iter().copied()),
            )
            .chain(fieldless)
            .collect();
        for i in order {
            let measure = &self.measures[i];
            match measure.op {
                Op::Values => {
                    let column = groups
                        .rows
                        .iter()
                        .map(|rows| Value::Array(rows.iter().map(|&row| data.row(row)).collect()))
                        .collect();
                    table = table.with(measure.output.clone(), Column::Value(column));
                }
                Op::Argmin | Op::Argmax => {
                    for (field, column) in data.fields() {
                        let column = column.pick(&picked[i]);
                        table = table.with(format!("{}.{field}", measure.output), column);
                    }
                }
                // The least or greatest text, as it is.
                Op::Min | Op::Max if !picked[i].is_empty() => {
                    let field = measure.field.as_deref().unwrap_or_default();
                    let Some(column) = data.field(field) else {
                        continue;
                    };
                    table = table.with(measure.output.clone(), column.pick(&picked[i]));
                }
                _ => table = table.number(measure.output.clone(), std::mem::take(&mut values[i])),
            }
        }
        table
    }

    /// Vega's `joinaggregate`: every row of `data`, each with what
    /// [`Self::apply`] measures of its group.
    pub fn join(&self, data: &Data, random: &mut Random) -> Data {
        let groups = Groups::of(data, &self.groupby);
        let table = self.apply(data, random);
        let mut owners = vec![0; data.len()];
        for (group, rows) in groups.rows.iter().enumerate() {
            for &row in rows {
                owners[row] = group;
            }
        }
        let mut out = data.clone();
        for (field, _) in table.fields() {
            if !self.groupby.contains(field) {
                out = out.adopt(&table, field.clone(), table.take(field, &owners));
            }
        }
        out
    }
}

/// Each row's value of `field`: a number column's `NaN`, null, undefined
/// and an empty string are missing, and any other value is valid, read as
/// JavaScript's `+value`.
fn cells(data: &Data, field: &str) -> Vec<Cell> {
    let missing = |value: &Value| match value {
        Value::String(text) => text.is_empty(),
        value => value.is_nullish(),
    };
    match data.field(field).as_deref() {
        Some(Column::Number(values)) => values
            .iter()
            .map(|&v| match v.is_nan() {
                true => Cell::Missing,
                false => Cell::Valid(v),
            })
            .collect(),
        Some(Column::Text(text)) => {
            let names: Vec<Cell> = text
                .names
                .iter()
                .map(|name| match name {
                    Name::Text(name) if !name.is_empty() => Cell::Valid(JsNumber::from(&**name).0),
                    _ => Cell::Missing,
                })
                .collect();
            text.codes
                .iter()
                .map(|&code| names[code as usize])
                .collect()
        }
        Some(Column::Value(values)) => values
            .iter()
            .map(|value| match missing(value) {
                true => Cell::Missing,
                false => Cell::Valid(value.to_number()),
            })
            .collect(),
        None => vec![Cell::Missing; data.len()],
    }
}

impl Running {
    /// `cells`' sums over `rows`, `text` the field's strings where it holds
    /// text.
    fn of(rows: &[usize], cells: &[Cell], text: Option<&Text>, exponent: f64) -> Self {
        // JavaScript's `<` on strings compares UTF-16 code units.
        let compare = |a: usize, b: usize, x: f64, y: f64| match text {
            Some(text) => Some(string::compare(text.get(a)?, text.get(b)?)),
            None => x.partial_cmp(&y),
        };
        let mut run = Running {
            valid: 0.0,
            missing: 0.0,
            sum: 0.0,
            product: 1.0,
            mean: 0.0,
            dev: 0.0,
            exp: 0.0,
            exponent,
            least: None,
            most: None,
            first: None,
        };
        for &row in rows {
            let v = match cells[row] {
                Cell::Missing => {
                    run.missing += 1.0;
                    continue;
                }
                Cell::Valid(v) => v,
            };
            run.valid += 1.0;
            run.first.get_or_insert(row);
            run.sum += v;
            run.product *= v;
            let delta = v - run.mean;
            run.mean += delta / run.valid;
            run.dev += delta * (v - run.mean);
            run.exp = exponent * run.exp + v;
            if text.is_some() || !v.is_nan() {
                let order = |(other, o): (usize, f64)| compare(row, other, v, o);
                if run
                    .least
                    .is_none_or(|least| order(least) == Some(Ordering::Less))
                {
                    run.least = Some((row, v));
                }
                if run
                    .most
                    .is_none_or(|most| order(most) == Some(Ordering::Greater))
                {
                    run.most = Some((row, v));
                }
            }
        }
        run
    }

    /// One op's value but `argmin`'s and `argmax`'s. `store` is the group's
    /// tuple store, and `field` which field's measures ask it.
    fn value(
        &self,
        op: Op,
        rows: &[usize],
        (cells, keys): (&[Cell], &[u64]),
        store: &mut Store,
        field: usize,
        random: &mut Random,
    ) -> Option<f64> {
        let valid = self.valid;
        let dev = number::max(0.0, self.dev);
        let some = |v: f64| (valid > 0.0).then_some(v);
        match op {
            Op::Count => Some(rows.len() as f64),
            Op::Valid => Some(valid),
            Op::Missing => Some(self.missing),
            Op::Sum => some(self.sum),
            Op::Product => some(self.product),
            Op::Mean | Op::Average => some(self.mean),
            Op::Variance => (valid > 1.0).then(|| dev / (valid - 1.0)),
            Op::Variancep => some(dev / valid),
            Op::Stdev => (valid > 1.0).then(|| (dev / (valid - 1.0)).sqrt()),
            Op::Stdevp => some((dev / valid).sqrt()),
            Op::Stderr => (valid > 1.0).then(|| (dev / (valid * (valid - 1.0))).sqrt()),
            Op::Min => self.least.map(|(_, v)| v),
            Op::Max => self.most.map(|(_, v)| v),
            Op::Exponential => {
                some(self.exp * (1.0 - self.exponent) / (1.0 - number::pow(self.exponent, valid)))
            }
            Op::Exponentialb => some(self.exp * (1.0 - self.exponent)),
            Op::Distinct => {
                store.get = None;
                let seen: HashSet<u64> = rows.iter().map(|&row| keys[row]).collect();
                Some(seen.len() as f64)
            }
            Op::Median | Op::Q1 | Op::Q3 => {
                if store.get != Some(field) || store.quartiles.is_none() {
                    let sorted = sorted(rows, cells);
                    store.quartiles =
                        Some([0.25, 0.5, 0.75].map(|p| array::quantile_sorted(&sorted, p)));
                    store.get = Some(field);
                }
                let [q1, q2, q3] = store.quartiles?;
                Some(match op {
                    Op::Q1 => q1,
                    Op::Q3 => q3,
                    _ => q2,
                })
            }
            Op::Ci0 | Op::Ci1 => {
                if store.get != Some(field) || store.ci.is_none() {
                    store.ci = Some(bootstrap(rows, cells, random));
                    store.get = Some(field);
                }
                let [low, high] = store.ci?;
                Some(if op == Op::Ci0 { low } else { high })
            }
            Op::Values | Op::Argmin | Op::Argmax => None,
        }
    }

    /// The row `argmin` or `argmax` picks.
    fn pick(&self, op: Op, store: &mut Store, field: usize) -> Option<usize> {
        let pick = match op {
            Op::Argmin => self.least,
            _ => self.most,
        };
        // Vega reads the row from its store when no later value beat the
        // first.
        if pick.map(|(row, _)| row) == self.first && !(store.get == Some(field) && store.extent) {
            store.extent = true;
            store.get = Some(field);
        }
        pick.map(|(row, _)| row)
    }
}

/// The valid numbers at `rows`, ascending.
fn sorted(rows: &[usize], cells: &[Cell]) -> Vec<f64> {
    let mut values = numbers(rows, cells);
    values.sort_by(f64::total_cmp);
    values
}

/// The valid numbers at `rows`, in row order.
fn numbers(rows: &[usize], cells: &[Cell]) -> Vec<f64> {
    rows.iter()
        .filter_map(|&row| match cells[row] {
            Cell::Valid(v) if !v.is_nan() => Some(v),
            _ => None,
        })
        .collect()
}

/// vega-statistics' `bootstrapCI(values, 1000, 0.05)`.
fn bootstrap(rows: &[usize], cells: &[Cell], random: &mut Random) -> [f64; 2] {
    let values = numbers(rows, cells);
    let n = values.len();
    let mut means: Vec<f64> = (0..SAMPLES)
        .map(|_| {
            let sum: f64 = random
                .by_ref()
                .take(n)
                .map(|draw| values[(draw * n as f64) as usize])
                .sum();
            sum / n as f64
        })
        .collect();
    means.retain(|v| !v.is_nan());
    means.sort_by(f64::total_cmp);
    [
        array::quantile_sorted(&means, ALPHA / 2.0),
        array::quantile_sorted(&means, 1.0 - ALPHA / 2.0),
    ]
}
