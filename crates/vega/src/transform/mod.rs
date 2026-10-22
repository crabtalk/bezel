//! vega-transforms' data transforms, run once over a data set's tuples as
//! Vega's dataflow runs them on its first pulse.

use crate::{
    Error,
    expr::Expr,
    path::Path,
    spec::{self, Compare, Field, Order, Transform},
    value::Value,
    view::View,
};
use std::{cmp::Ordering, collections::HashMap};

mod aggregate;
mod bin;
mod countpattern;
mod density;
mod impute;
mod regression;
mod simple;
mod stack;
mod timeunit;
mod window;

/// A datum and its `tupleid`: ids count up from 1 across a view in the
/// order tuples are ingested or derived.
#[derive(Clone, Debug)]
pub struct Tuple {
    pub id: u64,
    pub datum: Value,
}

/// A field parameter resolved: vega-util's `field` accessor of a path, or an
/// expression's.
#[derive(Clone)]
pub(crate) struct Accessor {
    get: Get,
    /// `accessorName`: the `as` name, else the field as written, a path of
    /// one key unescaped.
    pub name: String,
}

#[derive(Clone)]
enum Get {
    Path(Path),
    Expr(Expr),
    /// Pivot's: `value` where `key` is `k`, else `NaN`.
    Pivot {
        key: Box<Accessor>,
        value: Box<Accessor>,
        k: Value,
    },
}

impl Accessor {
    pub fn get(&self, view: &View, datum: &Value) -> Result<Value, Error> {
        match &self.get {
            Get::Path(path) => path.get(datum).map_err(|e| Error::Eval(e.into())),
            Get::Expr(expr) => view.eval(expr, datum),
            Get::Pivot { key, value, k } => match key.get(view, datum)?.strict_eq(k) {
                true => value.get(view, datum),
                false => Ok(Value::Number(f64::NAN)),
            },
        }
    }

    /// vega-util's `field(name)`.
    pub fn field(name: &str) -> Result<Self, Error> {
        let path: Path = name.parse().map_err(Error::Spec)?;
        let name = match path.len() {
            1 => path[0].clone(),
            _ => name.to_string(),
        };
        Ok(Self {
            get: Get::Path(path),
            name,
        })
    }

    pub fn new(view: &View, field: &Field) -> Result<Self, Error> {
        Ok(match field {
            Field::Name(name) => Self::field(name)?,
            Field::Signal(signal) => Self::field(&view.signal_ref(signal)?.to_string())?,
            Field::Field { field, as_ } => {
                let mut accessor = Self::field(field)?;
                if let Some(as_) = as_ {
                    accessor.name = as_.clone();
                }
                accessor
            }
            Field::Expr { expr, as_ } => Self {
                get: Get::Expr(view.parse(expr)?),
                name: as_.clone().unwrap_or_default(),
            },
        })
    }

    pub fn many(view: &View, fields: &[Field]) -> Result<Vec<Self>, Error> {
        fields.iter().map(|f| Self::new(view, f)).collect()
    }
}

/// What a transform does to its pulse, as its definition's `metadata` says.
pub(crate) struct Metadata {
    pub generates: bool,
    pub modifies: bool,
}

impl Transform {
    pub(crate) fn metadata(&self) -> Metadata {
        let (generates, modifies) = match self {
            Self::Aggregate(_)
            | Self::Countpattern(_)
            | Self::Cross(_)
            | Self::Density(_)
            | Self::Flatten(_)
            | Self::Fold(_)
            | Self::Kde(_)
            | Self::Loess(_)
            | Self::Pivot(_)
            | Self::Project(_)
            | Self::Quantile(_)
            | Self::Regression(_)
            | Self::Sequence(_) => (true, false),
            Self::Bin(_)
            | Self::Dotbin(_)
            | Self::Formula(_)
            | Self::Identifier(_)
            | Self::Joinaggregate(_)
            | Self::Lookup(_)
            | Self::Stack(_)
            | Self::Timeunit(_)
            | Self::Window(_) => (false, true),
            Self::Collect(_)
            | Self::Extent(_)
            | Self::Filter(_)
            | Self::Impute(_)
            | Self::Sample(_) => (false, false),
        };
        Metadata {
            generates,
            modifies,
        }
    }

    /// The name of the signal that holds the transform's value.
    pub(crate) fn signal(&self) -> Option<&str> {
        match self {
            Self::Aggregate(t) => t.signal.as_deref(),
            Self::Bin(t) => t.signal.as_deref(),
            Self::Collect(t) => t.signal.as_deref(),
            Self::Countpattern(t) => t.signal.as_deref(),
            Self::Cross(t) => t.signal.as_deref(),
            Self::Density(t) => t.signal.as_deref(),
            Self::Dotbin(t) => t.signal.as_deref(),
            Self::Extent(t) => t.signal.as_deref(),
            Self::Filter(t) => t.signal.as_deref(),
            Self::Flatten(t) => t.signal.as_deref(),
            Self::Fold(t) => t.signal.as_deref(),
            Self::Formula(t) => t.signal.as_deref(),
            Self::Identifier(t) => t.signal.as_deref(),
            Self::Impute(t) => t.signal.as_deref(),
            Self::Joinaggregate(t) => t.signal.as_deref(),
            Self::Kde(t) => t.signal.as_deref(),
            Self::Loess(t) => t.signal.as_deref(),
            Self::Lookup(t) => t.signal.as_deref(),
            Self::Pivot(t) => t.signal.as_deref(),
            Self::Project(t) => t.signal.as_deref(),
            Self::Quantile(t) => t.signal.as_deref(),
            Self::Regression(t) => t.signal.as_deref(),
            Self::Sample(t) => t.signal.as_deref(),
            Self::Sequence(t) => t.signal.as_deref(),
            Self::Stack(t) => t.signal.as_deref(),
            Self::Timeunit(t) => t.signal.as_deref(),
            Self::Window(t) => t.signal.as_deref(),
        }
    }

    /// The tuples the transform makes of `tuples`, and its value where it
    /// has one.
    pub(crate) fn run(
        &self,
        view: &View,
        tuples: Vec<Tuple>,
    ) -> Result<(Vec<Tuple>, Option<Value>), Error> {
        Ok(match self {
            Self::Aggregate(def) => (aggregate::aggregate(view, def, &tuples)?, None),
            Self::Joinaggregate(def) => (aggregate::join(view, def, tuples)?, None),
            Self::Bin(def) => {
                let (tuples, bins) = bin::bin(view, def, tuples)?;
                (tuples, Some(bins))
            }
            Self::Impute(def) => (impute::impute(view, def, tuples)?, None),
            Self::Pivot(def) => (impute::pivot(view, def, &tuples)?, None),
            Self::Density(def) => (density::density(view, def, &tuples)?, None),
            Self::Kde(def) => (density::kde(view, def, &tuples)?, None),
            Self::Loess(def) => (regression::loess(view, def, &tuples)?, None),
            Self::Quantile(def) => (regression::quantile(view, def, &tuples)?, None),
            Self::Regression(def) => (regression::regression(view, def, &tuples)?, None),
            Self::Countpattern(def) => (countpattern::countpattern(view, def, &tuples)?, None),
            Self::Dotbin(def) => {
                let (tuples, bins) = countpattern::dotbin(view, def, tuples)?;
                (tuples, Some(bins))
            }
            Self::Collect(def) => (simple::collect(view, def, tuples)?, None),
            Self::Cross(def) => (simple::cross(view, def, &tuples)?, None),
            Self::Extent(def) => {
                let extent = simple::extent(view, def, &tuples)?;
                (tuples, Some(extent))
            }
            Self::Filter(def) => (simple::filter(view, def, tuples)?, None),
            Self::Flatten(def) => (simple::flatten(view, def, &tuples)?, None),
            Self::Fold(def) => (simple::fold(view, def, &tuples)?, None),
            Self::Formula(def) => (simple::formula(view, def, tuples)?, None),
            Self::Identifier(def) => (simple::identifier(view, def, tuples), None),
            Self::Lookup(def) => (simple::lookup(view, def, tuples)?, None),
            Self::Project(def) => (simple::project(view, def, &tuples)?, None),
            Self::Sample(def) => (simple::sample(view, def, tuples)?, None),
            Self::Sequence(def) => (simple::sequence(view, def, tuples)?, None),
            Self::Stack(def) => (stack::stack(view, def, tuples)?, None),
            Self::Window(def) => (window::window(view, def, tuples)?, None),
            Self::Timeunit(def) => (timeunit::timeunit(view, def, tuples)?, None),
        })
    }
}

/// vega-util's `compare` over `def`'s fields, ties broken by `tupleid` as
/// vega-dataflow's `stableCompare` breaks them; `None` without fields.
pub(crate) struct Comparator {
    fields: Vec<(Accessor, Order)>,
}

impl Comparator {
    pub fn new(def: &Compare) -> Result<Option<Self>, Error> {
        let orders = def.order.as_ref().map(|o| o.items()).unwrap_or_default();
        let fields = def
            .field
            .as_ref()
            .map(|f| f.items())
            .unwrap_or_default()
            .iter()
            .enumerate()
            .map(|(i, field)| {
                let order = orders.get(i).copied().unwrap_or(Order::Ascending);
                Ok((Accessor::field(field)?, order))
            })
            .collect::<Result<Vec<_>, Error>>()?;
        Ok((!fields.is_empty()).then_some(Self { fields }))
    }

    /// The order of `a` and `b`, without the `tupleid` tie break.
    pub fn compare(&self, view: &View, a: &Value, b: &Value) -> Result<Ordering, Error> {
        for (field, order) in &self.fields {
            let c = field.get(view, a)?.ascending(&field.get(view, b)?);
            if c != Ordering::Equal {
                return Ok(match order {
                    Order::Ascending => c,
                    Order::Descending => c.reverse(),
                });
            }
        }
        Ok(Ordering::Equal)
    }

    /// `tuples` sorted, stably by `tupleid`.
    pub fn sort(&self, view: &View, tuples: &mut [Tuple]) -> Result<(), Error> {
        let mut order: Vec<usize> = (0..tuples.len()).collect();
        let all: &[Tuple] = tuples;
        self.sort_by(view, &mut order, |&i| &all[i])?;
        let sorted: Vec<Tuple> = order.iter().map(|&i| all[i].clone()).collect();
        tuples.clone_from_slice(&sorted);
        Ok(())
    }

    /// `items` sorted by the tuple each names, stably by `tupleid`.
    pub fn sort_by<'t, T>(
        &self,
        view: &View,
        items: &mut [T],
        tuple: impl Fn(&T) -> &'t Tuple,
    ) -> Result<(), Error> {
        let mut error = None;
        items.sort_by(|a, b| {
            let (a, b) = (tuple(a), tuple(b));
            match self.compare(view, &a.datum, &b.datum) {
                Ok(Ordering::Equal) => a.id.cmp(&b.id),
                Ok(order) => order,
                Err(e) => {
                    error.get_or_insert(e);
                    Ordering::Equal
                }
            }
        });
        error.map_or(Ok(()), Err)
    }
}

/// How [`groups`] keys a tuple by its `groupby` values.
#[derive(Clone, Copy)]
pub(crate) enum Key {
    /// vega-transforms' `partition`: the values' array as `String` writes
    /// it, null and undefined empty, joined by `,`.
    Partition,
    /// vega-util's `key`: each value's `'' + value`, joined by `|`.
    Cells,
}

/// Tuples grouped by their `groupby` values, groups in the order of their
/// first tuple.
pub(crate) fn groups(
    view: &View,
    groupby: &[Accessor],
    key: Key,
    tuples: &[Tuple],
) -> Result<Vec<Vec<usize>>, Error> {
    let mut index: HashMap<String, usize> = HashMap::new();
    let mut groups: Vec<Vec<usize>> = Vec::new();
    for (i, t) in tuples.iter().enumerate() {
        let values = groupby
            .iter()
            .map(|f| f.get(view, &t.datum))
            .collect::<Result<Vec<_>, _>>()?;
        let text = match key {
            Key::Partition => Value::from(values).to_string(),
            Key::Cells => values
                .iter()
                .map(Value::to_string)
                .collect::<Vec<_>>()
                .join("|"),
        };
        let group = *index.entry(text).or_insert_with(|| {
            groups.push(Vec::new());
            groups.len() - 1
        });
        groups[group].push(i);
    }
    Ok(groups)
}

/// A group of tuples and its `groupby` values, as `partition` gives them.
pub(crate) struct Group {
    pub dims: Vec<Value>,
    pub rows: Vec<usize>,
}

/// vega-transforms' and vega-regression's `partition`: [`groups`] keyed as
/// arrays, each with the values of its first tuple.
pub(crate) fn partition(
    view: &View,
    groupby: &[Accessor],
    tuples: &[Tuple],
) -> Result<Vec<Group>, Error> {
    groups(view, groupby, Key::Partition, tuples)?
        .into_iter()
        .map(|rows| {
            let dims = groupby
                .iter()
                .map(|g| g.get(view, &tuples[rows[0]].datum))
                .collect::<Result<Vec<_>, _>>()?;
            Ok(Group { dims, rows })
        })
        .collect()
}

/// The value of an optional parameter, `default` where it is not written.
pub(crate) fn or<T: Clone + serde::de::DeserializeOwned>(
    view: &View,
    param: &Option<spec::Param<T>>,
    default: T,
) -> Result<T, Error> {
    param.as_ref().map_or(Ok(default), |p| view.param(p))
}
