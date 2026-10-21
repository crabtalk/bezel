//! Statistical transforms over a [`Data`], as Vega-Lite's `transform` ops
//! compute them. Each op reads fields by name and answers a new table; a field
//! the table lacks reads as missing values.

use crate::data::{Column, Data};
pub use density::{Density, Resolve};
use gpui::SharedString;
pub use loess::Loess;
pub use quantile::Quantile;
pub use regression::{Method, Regression};
use std::{collections::HashMap, sync::Arc};

mod curve;
mod density;
mod loess;
mod quantile;
mod regression;

/// One op of a pipeline. [`Data::transform`] runs a list of them in order.
#[derive(Clone, Debug, PartialEq)]
pub enum Transform {
    Density(Density),
    Loess(Loess),
    Regression(Regression),
    Quantile(Quantile),
}

impl Transform {
    pub fn apply(&self, data: &Data) -> Data {
        match self {
            Self::Density(op) => op.apply(data),
            Self::Loess(op) => op.apply(data),
            Self::Regression(op) => op.apply(data),
            Self::Quantile(op) => op.apply(data),
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

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum Key {
    Missing,
    Number(u64),
    Text(u32),
}

impl<'a> Groups<'a> {
    fn of(data: &Data, fields: &'a [SharedString]) -> Self {
        let columns: Vec<Option<&Column>> = fields.iter().map(|f| data.column(f)).collect();
        let mut index: HashMap<Vec<Key>, usize> = HashMap::new();
        let mut rows: Vec<Vec<usize>> = Vec::new();
        for row in 0..data.len() {
            let key = columns
                .iter()
                .map(|column| match column {
                    None => Key::Missing,
                    Some(Column::Number(values)) if values[row].is_nan() => Key::Missing,
                    // `+ 0.0` makes -0 key as 0.
                    Some(Column::Number(values)) => Key::Number((values[row] + 0.0).to_bits()),
                    Some(Column::Text(text)) => Key::Text(text.codes[row]),
                })
                .collect();
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
        let mut table = Data::new();
        for field in self.fields {
            let column = match data.column(field) {
                Some(Column::Text(text)) => {
                    Column::Text(owners.iter().map(|row| text.get(*row).clone()).collect())
                }
                Some(Column::Number(values)) => {
                    Column::Number(owners.iter().map(|row| values[*row]).collect())
                }
                None => Column::Number(Arc::from(vec![f64::NAN; owners.len()])),
            };
            table = table.with(field.clone(), column);
        }
        for (i, field) in output.iter().enumerate() {
            let values: Vec<f64> = points.iter().flatten().map(|point| point[i]).collect();
            table = table.number(field.clone(), values);
        }
        table
    }
}
