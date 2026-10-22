//! A table of named, equal-length columns.

use crate::{time, transform::Transform};
use d3::format::number::Number as JsNumber;
use gpui::SharedString;
use std::{
    borrow::Cow,
    cell::RefCell,
    collections::HashMap,
    fmt,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};
use vega::{
    expr::{self, Context, Expr},
    path::Path,
    statistics::Random,
    value::Value,
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
    /// JavaScript values of any type, arrays and objects among them.
    Value(Arc<[Value]>),
}

/// Values stored as codes into `names`, each distinct value once, in the
/// order first seen.
#[derive(Clone, Debug)]
pub struct Text {
    pub codes: Arc<[u32]>,
    pub names: Arc<[Name]>,
}

/// A value of a text column.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Name {
    Text(SharedString),
    Null,
    /// A missing field's.
    Undefined,
}

impl Text {
    /// `None` for null and undefined.
    pub fn get(&self, row: usize) -> Option<&SharedString> {
        match &self.names[self.codes[row] as usize] {
            Name::Text(text) => Some(text),
            Name::Null | Name::Undefined => None,
        }
    }
}

impl FromIterator<Name> for Text {
    fn from_iter<I: IntoIterator<Item = Name>>(values: I) -> Self {
        let mut index = HashMap::new();
        let mut names = Vec::new();
        let codes = values
            .into_iter()
            .map(|value| {
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

/// `None` is null.
impl<S: Into<SharedString>> FromIterator<Option<S>> for Text {
    fn from_iter<I: IntoIterator<Item = Option<S>>>(values: I) -> Self {
        values
            .into_iter()
            .map(|value| value.map_or(Name::Null, |text| Name::Text(text.into())))
            .collect()
    }
}

/// Each value's `String(value)` but null's and undefined's.
impl From<&[Value]> for Text {
    fn from(values: &[Value]) -> Self {
        values.iter().map(Name::from).collect()
    }
}

/// A string, or any other value but null and undefined, as `String(value)`
/// writes it.
impl From<&Value> for Name {
    fn from(value: &Value) -> Self {
        match value {
            Value::Null => Name::Null,
            Value::Undefined => Name::Undefined,
            Value::String(text) => Name::Text(text.to_string().into()),
            value => Name::Text(value.to_string().into()),
        }
    }
}

impl From<&Name> for Value {
    fn from(name: &Name) -> Self {
        match name {
            Name::Text(text) => Value::String(text.as_ref().into()),
            Name::Null => Value::Null,
            Name::Undefined => Value::Undefined,
        }
    }
}

/// JavaScript's `String(value)`.
impl fmt::Display for Name {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Name::Text(text) => f.write_str(text),
            name => Value::from(name).fmt(f),
        }
    }
}

/// Numbers where every value is a number, null or undefined; text where
/// every value is a string, null or undefined; the values as they are
/// otherwise. A number column holds null and undefined as `NaN`.
impl FromIterator<Value> for Column {
    fn from_iter<I: IntoIterator<Item = Value>>(values: I) -> Self {
        let values: Vec<Value> = values.into_iter().collect();
        let all = |kind: fn(&Value) -> bool| values.iter().all(|v| v.is_nullish() || kind(v));
        if all(|v| matches!(v, Value::Number(_))) {
            Self::Number(
                values
                    .iter()
                    .map(|v| match v {
                        Value::Number(n) => *n,
                        _ => f64::NAN,
                    })
                    .collect(),
            )
        } else if all(|v| matches!(v, Value::String(_))) {
            Self::Text(Text::from(&values[..]))
        } else {
            Self::Value(values.into())
        }
    }
}

impl Column {
    /// Its numbers, `None` for text and values.
    pub fn numbers(&self) -> Option<Arc<[f64]>> {
        match self {
            Column::Number(values) => Some(values.clone()),
            Column::Text(_) | Column::Value(_) => None,
        }
    }

    /// Row `row`'s value: a number column's `NaN` is null.
    pub fn value(&self, row: usize) -> Value {
        match self {
            Self::Number(values) if values[row].is_nan() => Value::Null,
            Self::Number(values) => Value::Number(values[row]),
            Self::Text(text) => Value::from(&text.names[text.codes[row] as usize]),
            Self::Value(values) => values[row].clone(),
        }
    }

    /// Row `row` as a discrete scale names it, `String(value)`: a number
    /// column's `NaN` as `null`.
    pub fn label(&self, row: usize) -> SharedString {
        match self.value(row) {
            Value::String(text) => text.into(),
            value => value.to_string().into(),
        }
    }

    /// Its values at `rows`, in that order: null for `None` in a text
    /// column, `NaN` in a number column, undefined in a column of values.
    pub fn pick(&self, rows: &[Option<usize>]) -> Column {
        match self {
            Self::Number(values) => Self::Number(
                rows.iter()
                    .map(|row| row.map_or(f64::NAN, |r| values[r]))
                    .collect(),
            ),
            Self::Text(text) => Self::Text(
                rows.iter()
                    .map(|row| {
                        row.map_or(Name::Null, |r| text.names[text.codes[r] as usize].clone())
                    })
                    .collect(),
            ),
            Self::Value(values) => Self::Value(
                rows.iter()
                    .map(|row| row.map_or(Value::Undefined, |r| values[r].clone()))
                    .collect(),
            ),
        }
    }

    pub fn len(&self) -> usize {
        match self {
            Self::Number(values) => values.len(),
            Self::Text(text) => text.codes.len(),
            Self::Value(values) => values.len(),
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

    /// Each row's value of `field` as a [`key`], text by its code, and any
    /// other value by its `String(value)`, as Vega keys it. `u64::MAX` for a
    /// missing number, and throughout for a field the table lacks.
    pub fn keys(&self, field: &str) -> Vec<u64> {
        let date = self.is_time(field);
        match self.field(field).as_deref() {
            Some(Column::Number(values)) => values.iter().map(|&v| key(v, date)).collect(),
            Some(Column::Text(text)) => text.codes.iter().map(|&code| u64::from(code)).collect(),
            Some(Column::Value(values)) => {
                let mut index = HashMap::new();
                values
                    .iter()
                    .map(|value| {
                        let next = index.len() as u64;
                        *index.entry(value.to_string()).or_insert(next)
                    })
                    .collect()
            }
            None => vec![u64::MAX; self.len()],
        }
    }

    pub fn text<S: Into<SharedString>>(
        self,
        field: impl Into<SharedString>,
        values: impl IntoIterator<Item = S>,
    ) -> Self {
        self.with(field, Column::Text(values.into_iter().map(Some).collect()))
    }

    /// The column named `field`.
    pub fn column(&self, field: &str) -> Option<&Column> {
        self.0
            .fields
            .iter()
            .find(|(name, _)| name == field)
            .map(|(_, column)| column)
    }

    /// The column named `field`, or else the values at `field` read as a
    /// path into a column, as Vega's field accessor reads it: `a.b` is each
    /// row's `a`'s `b`, undefined where `a` is null or undefined.
    pub fn field(&self, field: &str) -> Option<Cow<'_, Column>> {
        if let Some(column) = self.column(field) {
            return Some(Cow::Borrowed(column));
        }
        let path: Path = field.parse().ok()?;
        let (first, rest) = path.split_first()?;
        let column = self.column(first)?;
        if rest.is_empty() {
            return Some(Cow::Borrowed(column));
        }
        let rest = Path::from(rest);
        Some(Cow::Owned(
            (0..self.len())
                .map(|row| rest.get(&column.value(row)).unwrap_or_default())
                .collect(),
        ))
    }

    /// `field`'s values as numbers, any other value read as JavaScript's
    /// `+value` reads it. `NaN` for null, undefined, an unreadable value, an
    /// empty string, and every row of a field the table lacks.
    pub fn numbers(&self, field: &str) -> Vec<f64> {
        let text = |name: &str| match name.is_empty() {
            true => f64::NAN,
            false => JsNumber::from(name).0,
        };
        match self.field(field).as_deref() {
            Some(Column::Number(values)) => values.to_vec(),
            Some(Column::Text(text_column)) => {
                let names: Vec<f64> = text_column
                    .names
                    .iter()
                    .map(|name| match name {
                        Name::Text(name) => text(name),
                        Name::Null | Name::Undefined => f64::NAN,
                    })
                    .collect();
                text_column
                    .codes
                    .iter()
                    .map(|code| names[*code as usize])
                    .collect()
            }
            Some(Column::Value(values)) => values
                .iter()
                .map(|value| match value {
                    Value::String(name) => text(name),
                    value if value.is_nullish() => f64::NAN,
                    value => value.to_number(),
                })
                .collect(),
            None => vec![f64::NAN; self.len()],
        }
    }

    /// `field`'s values at `rows`, in that order. `NaN` throughout for a field
    /// the table lacks.
    pub fn take(&self, field: &str, rows: &[usize]) -> Column {
        match self.field(field) {
            Some(column) => column.pick(&rows.iter().copied().map(Some).collect::<Vec<_>>()),
            None => Column::Number(vec![f64::NAN; rows.len()].into()),
        }
    }

    /// The table of its rows at `rows`, in that order.
    pub fn select(&self, rows: &[usize]) -> Data {
        let picked: Vec<Option<usize>> = rows.iter().copied().map(Some).collect();
        self.fields().fold(Data::new(), |data, (name, column)| {
            data.adopt(self, name.clone(), column.pick(&picked))
        })
    }

    /// `expr` on row `row` as its `datum`, with no params, scales or data
    /// sets, `random()` drawing from `random`.
    pub fn eval(
        &self,
        expr: &Expr,
        row: usize,
        random: &RefCell<Random>,
    ) -> Result<Value, expr::Error> {
        let datum = self.row(row);
        let context = Context {
            datum: &datum,
            param: &|_| Value::Undefined,
            scale: &|_| None,
            data: &|_| Vec::new(),
            random,
        };
        expr.eval(&context)
    }

    /// Row `row` as a JavaScript object of every field, in field order.
    pub fn row(&self, row: usize) -> Value {
        Value::Object(
            self.fields()
                .map(|(name, column)| (name.clone().into(), column.value(row)))
                .collect(),
        )
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
