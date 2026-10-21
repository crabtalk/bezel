//! Data a channel maps through its scale: a field's values or one datum.

use std::sync::Arc;

use gpui::SharedString;

use crate::{
    data::{Column, Data},
    scale, time,
};

/// A field, or a datum, read as `kind` and placed by `scale`.
#[derive(Clone, Debug, PartialEq)]
pub struct Channel {
    pub source: Source,
    pub kind: Kind,
    /// The axis's or legend's title. `None` shows none.
    pub title: Option<SharedString>,
    pub scale: Scale,
    /// The order of a discrete field's values. On `order`, the direction rows
    /// are drawn in, whatever the kind.
    pub sort: Sort,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Source {
    /// Each row's value of this field.
    Field(SharedString),
    /// This value for every row, in the field's domain.
    Datum(Datum),
}

/// A value in a field's domain. A time is milliseconds since the epoch.
#[derive(Clone, Debug, PartialEq)]
pub enum Datum {
    Number(f64),
    Text(SharedString),
}

/// How a continuous field's values map to positions.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Scale {
    /// Whether a quantitative scale takes in zero. A stack of bars always
    /// starts at zero.
    pub zero: bool,
    /// Fixed `[low, high]`, times in milliseconds since the epoch. Marks
    /// outside it are drawn past the plot unless their layer has `clip`.
    pub domain: Option<[f64; 2]>,
    /// Whether a scale with no fixed domain widens to the ticks either side.
    pub nice: bool,
}

impl Default for Scale {
    fn default() -> Self {
        Self {
            zero: true,
            domain: None,
            nice: true,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub enum Sort {
    /// The order rows first show each value in.
    #[default]
    Data,
    Ascending,
    Descending,
    /// These values first, in this order, then the rest in data order. A
    /// number matches as its read-out text.
    Explicit(Vec<SharedString>),
}

/// How a field's values are read, whatever column holds them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Kind {
    Quantitative,
    Temporal,
    Ordinal,
    Nominal,
}

impl Kind {
    /// Ordinal or nominal.
    pub fn is_discrete(self) -> bool {
        matches!(self, Self::Ordinal | Self::Nominal)
    }

    /// A number in a field of this kind, read out: a time as a date, any
    /// other as [`scale::value`] writes it.
    pub fn describe(self, value: f64) -> String {
        match self {
            Self::Temporal => time::describe(value),
            _ => scale::value(value),
        }
    }
}

impl From<f64> for Datum {
    fn from(value: f64) -> Self {
        Self::Number(value)
    }
}

impl From<&str> for Datum {
    fn from(value: &str) -> Self {
        Self::Text(value.to_string().into())
    }
}

impl Channel {
    pub fn new(field: impl Into<SharedString>, kind: Kind) -> Self {
        Self::of(Source::Field(field.into()), kind)
    }

    /// `datum` for every row.
    pub fn datum(datum: impl Into<Datum>, kind: Kind) -> Self {
        Self::of(Source::Datum(datum.into()), kind)
    }

    fn of(source: Source, kind: Kind) -> Self {
        Self {
            source,
            kind,
            title: None,
            scale: Scale::default(),
            sort: Sort::default(),
        }
    }

    pub fn quantitative(field: impl Into<SharedString>) -> Self {
        Self::new(field, Kind::Quantitative)
    }

    pub fn temporal(field: impl Into<SharedString>) -> Self {
        Self::new(field, Kind::Temporal)
    }

    pub fn ordinal(field: impl Into<SharedString>) -> Self {
        Self::new(field, Kind::Ordinal)
    }

    pub fn nominal(field: impl Into<SharedString>) -> Self {
        Self::new(field, Kind::Nominal)
    }

    pub fn title(mut self, title: impl Into<SharedString>) -> Self {
        self.title = Some(title.into());
        self
    }

    pub fn zero(mut self, zero: bool) -> Self {
        self.scale.zero = zero;
        self
    }

    pub fn domain(mut self, low: f64, high: f64) -> Self {
        self.scale.domain = Some([low, high]);
        self
    }

    pub fn nice(mut self, nice: bool) -> Self {
        self.scale.nice = nice;
        self
    }

    pub fn sort(mut self, sort: Sort) -> Self {
        self.sort = sort;
        self
    }

    /// The field it reads, if it reads one.
    pub fn field(&self) -> Option<&SharedString> {
        match &self.source {
            Source::Field(field) => Some(field),
            Source::Datum(_) => None,
        }
    }

    /// Its value in each of `data`'s rows. `None` for a field `data` lacks.
    pub fn column(&self, data: &Data) -> Option<Column> {
        match &self.source {
            Source::Field(field) => data.column(field).cloned(),
            Source::Datum(Datum::Number(value)) => {
                Some(Column::Number(vec![*value; data.len()].into()))
            }
            Source::Datum(Datum::Text(text)) => Some(Column::Text(
                std::iter::repeat_n(text.clone(), data.len()).collect(),
            )),
        }
    }

    /// Its numbers in each of `data`'s rows. `None` for a field `data` lacks
    /// or holds as text.
    pub fn numbers(&self, data: &Data) -> Option<Arc<[f64]>> {
        match self.column(data)? {
            Column::Number(values) => Some(values),
            Column::Text(_) => None,
        }
    }
}
