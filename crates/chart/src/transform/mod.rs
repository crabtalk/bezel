//! Transforms over a [`Data`], as Vega-Lite's `transform` ops compute
//! them. Each op reads a field as [`Data::field`] reads it and answers a new
//! table; a field the table lacks reads as missing values.

use crate::data::Data;
pub use aggregate::{Aggregate, Measure, Op};
pub use bin::{Bin, Bins};
pub use density::{Density, Resolve};
use gpui::SharedString;
pub use loess::Loess;
pub use quantile::Quantile;
pub use regression::Regression;
use std::collections::HashMap;
pub use timeunit::{Part, TimeUnit};
use vega::statistics::Random;

mod aggregate;
mod bin;
mod density;
mod loess;
mod quantile;
mod regression;
mod timeunit;

/// One op of a pipeline. [`Data::transform`] runs a list of them in order.
#[derive(Clone, Debug, PartialEq)]
pub enum Transform {
    Density(Density),
    Loess(Loess),
    Regression(Regression),
    Quantile(Quantile),
    Aggregate(Aggregate),
    Bin(Bin),
    TimeUnit(TimeUnit),
}

impl Transform {
    /// The table this op makes of `data`, drawing from `random` where it
    /// draws.
    pub fn apply(&self, data: &Data, random: &mut Random) -> Data {
        match self {
            Self::Density(op) => op.apply(data),
            Self::Loess(op) => op.apply(data),
            Self::Regression(op) => op.apply(data),
            Self::Quantile(op) => op.apply(data),
            Self::Aggregate(op) => op.apply(data, random),
            Self::Bin(op) => op.apply(data),
            Self::TimeUnit(op) => op.apply(data),
        }
    }
}

/// Rows split by their values of some fields, groups in the order their first
/// row comes. A row's values key it as Vega's `groupby` does: equal numbers,
/// equal text and missing values each fall together.
struct Groups<'a> {
    fields: &'a [SharedString],
    /// Each group's rows, ascending.
    rows: Vec<Vec<usize>>,
}

impl<'a> Groups<'a> {
    fn of(data: &Data, fields: &'a [SharedString]) -> Self {
        let keys: Vec<Vec<u64>> = fields.iter().map(|f| data.keys(f)).collect();
        let mut index: HashMap<Vec<u64>, usize> = HashMap::new();
        let mut rows: Vec<Vec<usize>> = Vec::new();
        for row in 0..data.len() {
            let key = keys.iter().map(|keys| keys[row]).collect();
            let group = *index.entry(key).or_insert_with(|| {
                rows.push(Vec::new());
                rows.len() - 1
            });
            rows[group].push(row);
        }
        Self { fields, rows }
    }

    /// One row per point, each group's `fields` taken from its first row,
    /// then the points' two values as `output`.
    fn table(&self, data: &Data, points: &[Vec<[f64; 2]>], output: &[SharedString; 2]) -> Data {
        let owners: Vec<usize> = points
            .iter()
            .enumerate()
            .flat_map(|(group, points)| std::iter::repeat_n(self.rows[group][0], points.len()))
            .collect();
        let mut table = self.keys(data, &owners);
        for (i, field) in output.iter().enumerate() {
            let values: Vec<f64> = points.iter().flatten().map(|point| point[i]).collect();
            table = table.number(field.clone(), values);
        }
        table
    }

    /// The `fields` of each of `owners`, rows of `data`.
    fn keys(&self, data: &Data, owners: &[usize]) -> Data {
        let mut table = Data::new();
        for field in self.fields {
            table = table.adopt(data, field.clone(), data.take(field, owners));
        }
        table
    }
}
