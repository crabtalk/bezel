//! A table of named, equal-length columns.

use crate::{
    time,
    transform::{Random, Transform},
};
use gpui::SharedString;
use js::number::Number as JsNumber;
use std::{
    collections::HashMap,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};

static GENERATIONS: AtomicU64 = AtomicU64::new(1);

/// Columns by name, each `len` rows long. Cloning is one reference count.
///
/// Every change takes a generation no other table has had, and a clone shares
/// its original's, so equal generations mean equal contents. A table rebuilt
/// from the same values is a new generation: hold the table, not the values
/// it was built from, to keep what was computed from it.
#[derive(Clone, Debug, Default)]
pub struct Data(Arc<Table>);

#[derive(Clone, Debug, Default)]
struct Table {
    fields: Vec<(SharedString, Column)>,
    /// The fields whose numbers are dates, as Vega holds them.
    times: Vec<SharedString>,
    len: usize,
    generation: u64,
}

#[derive(Clone, Debug)]
pub enum Column {
    /// Quantities and times. A time is milliseconds since the Unix epoch, UTC.
    /// `NaN` is a missing value.
    Number(Arc<[f64]>),
    Text(Text),
}

/// Strings stored as codes into `names`, each distinct string once, in the
/// order first seen.
#[derive(Clone, Debug)]
pub struct Text {
    pub codes: Arc<[u32]>,
    pub names: Arc<[SharedString]>,
}

impl Text {
    pub fn get(&self, row: usize) -> &SharedString {
        &self.names[self.codes[row] as usize]
    }
}

impl<S: Into<SharedString>> FromIterator<S> for Text {
    fn from_iter<I: IntoIterator<Item = S>>(values: I) -> Self {
        let mut index = HashMap::new();
        let mut names = Vec::new();
        let codes = values
            .into_iter()
            .map(|value| {
                let value = value.into();
                *index.entry(value.clone()).or_insert_with(|| {
                    names.push(value);
                    names.len() as u32 - 1
                })
            })
            .collect();
        Self {
            codes,
            names: names.into(),
        }
    }
}

impl Column {
    /// Its numbers, `None` for text.
    pub fn numbers(&self) -> Option<Arc<[f64]>> {
        match self {
            Column::Number(values) => Some(values.clone()),
            Column::Text(_) => None,
        }
    }

    pub fn len(&self) -> usize {
        match self {
            Self::Number(values) => values.len(),
            Self::Text(text) => text.codes.len(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// The key two numbers share where Vega's `groupby` puts them together:
/// JavaScript writes them alike, -0 as 0 and a `date` to the second.
/// `u64::MAX` for `NaN`.
pub fn key(value: f64, date: bool) -> u64 {
    match (value.is_nan(), date) {
        (true, _) => u64::MAX,
        (false, true) => (time::second(value) + 0.0).to_bits(),
        (false, false) => (value + 0.0).to_bits(),
    }
}

impl Data {
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds `field`, or replaces the column of that name in place.
    ///
    /// # Panics
    ///
    /// If the table already has a column and `column` is not as long.
    pub fn with(mut self, field: impl Into<SharedString>, column: Column) -> Self {
        let field = field.into();
        let table = Arc::make_mut(&mut self.0);
        if let Some((name, other)) = table.fields.iter().find(|(name, _)| *name != field) {
            assert_eq!(
                column.len(),
                other.len(),
                "column `{field}` is not as long as `{name}`"
            );
        }
        table.len = column.len();
        table.times.retain(|name| *name != field);
        match table.fields.iter_mut().find(|(name, _)| *name == field) {
            Some((_, slot)) => *slot = column,
            None => table.fields.push((field, column)),
        }
        table.generation = GENERATIONS.fetch_add(1, Ordering::Relaxed);
        self
    }

    pub fn number(self, field: impl Into<SharedString>, values: impl Into<Arc<[f64]>>) -> Self {
        self.with(field, Column::Number(values.into()))
    }

    /// Adds `field` as dates, `values` their times: Vega keys a date by its
    /// text, to the second, where it groups.
    pub fn times(self, field: impl Into<SharedString>, values: impl Into<Arc<[f64]>>) -> Self {
        let field = field.into();
        let mut data = self.number(field.clone(), values);
        Arc::make_mut(&mut data.0).times.push(field);
        data
    }

    /// Whether `field` holds dates.
    pub fn is_time(&self, field: &str) -> bool {
        self.0.times.iter().any(|name| name == field)
    }

    /// Adds `field` as `column`, as dates where `from`'s `field` holds dates.
    pub fn adopt(self, from: &Data, field: impl Into<SharedString>, column: Column) -> Self {
        let field = field.into();
        match (from.is_time(&field), column) {
            (true, Column::Number(times)) => self.times(field, times),
            (_, column) => self.with(field, column),
        }
    }

    /// Each row's value of `field` as a [`key`], text by its code. `u64::MAX`
    /// for a missing value, and throughout for a field the table lacks.
    pub fn keys(&self, field: &str) -> Vec<u64> {
        let date = self.is_time(field);
        match self.column(field) {
            Some(Column::Number(values)) => values.iter().map(|&v| key(v, date)).collect(),
            Some(Column::Text(text)) => text.codes.iter().map(|&code| u64::from(code)).collect(),
            None => vec![u64::MAX; self.len()],
        }
    }

    pub fn text<S: Into<SharedString>>(
        self,
        field: impl Into<SharedString>,
        values: impl IntoIterator<Item = S>,
    ) -> Self {
        self.with(field, Column::Text(values.into_iter().collect()))
    }

    pub fn column(&self, field: &str) -> Option<&Column> {
        self.0
            .fields
            .iter()
            .find(|(name, _)| name == field)
            .map(|(_, column)| column)
    }

    /// `field`'s values as numbers, text read as JavaScript's `+text` reads
    /// it. `NaN` for a missing or unreadable value, an empty string, and every
    /// row of a field the table lacks.
    pub fn numbers(&self, field: &str) -> Vec<f64> {
        match self.column(field) {
            Some(Column::Number(values)) => values.to_vec(),
            Some(Column::Text(text)) => {
                let names: Vec<f64> = text
                    .names
                    .iter()
                    .map(|name| match name.is_empty() {
                        true => f64::NAN,
                        false => JsNumber::from(name.as_ref()).0,
                    })
                    .collect();
                text.codes
                    .iter()
                    .map(|code| names[*code as usize])
                    .collect()
            }
            None => vec![f64::NAN; self.len()],
        }
    }

    /// `field`'s values at `rows`, in that order. `NaN` throughout for a field
    /// the table lacks.
    pub fn take(&self, field: &str, rows: &[usize]) -> Column {
        match self.column(field) {
            Some(Column::Text(text)) => {
                Column::Text(rows.iter().map(|row| text.get(*row).clone()).collect())
            }
            Some(Column::Number(values)) => {
                Column::Number(rows.iter().map(|row| values[*row]).collect())
            }
            None => Column::Number(vec![f64::NAN; rows.len()].into()),
        }
    }

    /// The table `pipeline`'s ops make of this one, run in order, each
    /// drawing from `random` where it draws.
    pub fn transform(self, pipeline: &[Transform], random: &mut Random) -> Self {
        pipeline
            .iter()
            .fold(self, |data, op| op.apply(&data, random))
    }

    /// In the order they were first added.
    pub fn fields(&self) -> impl Iterator<Item = (&SharedString, &Column)> {
        self.0.fields.iter().map(|(name, column)| (name, column))
    }

    /// Rows. 0 for a table with no columns.
    pub fn len(&self) -> usize {
        self.0.len
    }

    pub fn is_empty(&self) -> bool {
        self.0.len == 0
    }

    /// 0 for a table with no columns.
    pub fn generation(&self) -> u64 {
        self.0.generation
    }
}
