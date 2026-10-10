//! A table of named, equal-length columns.

use std::{
    collections::HashMap,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};

use gpui::SharedString;

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
