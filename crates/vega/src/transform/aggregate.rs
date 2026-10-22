//! vega-transforms' `Aggregate` and `JoinAggregate`, with `AggregateOps`
//! and the per-cell `TupleStore`.

use crate::{
    Error,
    spec::{self, Field, Param},
    statistics,
    transform::{Accessor, Key, Tuple, groups, or},
    value::Value,
    view::View,
};
use std::{
    cmp::Ordering,
    collections::{HashMap, HashSet},
};

/// An aggregate op, by its Vega name.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Op {
    Values,
    Count,
    Missing,
    Valid,
    Sum,
    Product,
    Mean,
    Average,
    Variance,
    Variancep,
    Stdev,
    Stdevp,
    Stderr,
    Distinct,
    Ci0,
    Ci1,
    Median,
    Q1,
    Q3,
    Min,
    Max,
    Argmin,
    Argmax,
    Exponential,
    Exponentialb,
    /// Pivot's count: the valid and missing values.
    CountValues,
}

const OPS: [(Op, &str); 26] = [
    (Op::Values, "values"),
    (Op::Count, "count"),
    (Op::Missing, "missing"),
    (Op::Valid, "valid"),
    (Op::Sum, "sum"),
    (Op::Product, "product"),
    (Op::Mean, "mean"),
    (Op::Average, "average"),
    (Op::Variance, "variance"),
    (Op::Variancep, "variancep"),
    (Op::Stdev, "stdev"),
    (Op::Stdevp, "stdevp"),
    (Op::Stderr, "stderr"),
    (Op::Distinct, "distinct"),
    (Op::Ci0, "ci0"),
    (Op::Ci1, "ci1"),
    (Op::Median, "median"),
    (Op::Q1, "q1"),
    (Op::Q3, "q3"),
    (Op::Min, "min"),
    (Op::Max, "max"),
    (Op::Argmin, "argmin"),
    (Op::Argmax, "argmax"),
    (Op::Exponential, "exponential"),
    (Op::Exponentialb, "exponentialb"),
    (Op::CountValues, "__count__"),
];

impl Op {
    pub fn parse(name: &str) -> Result<Self, Error> {
        OPS.iter()
            .find(|(_, n)| *n == name)
            .map(|(op, _)| *op)
            .ok_or_else(|| Error::Spec(format!("Unrecognized aggregate op: {name}")))
    }

    pub fn name(self) -> &'static str {
        OPS.iter()
            .find(|(op, _)| *op == self)
            .map(|(_, n)| *n)
            .expect("every op is named")
    }

    /// The order ops compute in.
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

    /// The ops this one reads the state of.
    fn requires(self) -> &'static [Op] {
        match self {
            Op::Average | Op::Variance => &[Op::Mean],
            Op::Variancep | Op::Stdev | Op::Stdevp | Op::Stderr => &[Op::Variance],
            Op::Distinct | Op::Ci0 | Op::Ci1 | Op::Median | Op::Q1 | Op::Q3 | Op::Min | Op::Max => {
                &[Op::Values]
            }
            Op::Argmin => &[Op::Min, Op::Values],
            Op::Argmax => &[Op::Max, Op::Values],
            Op::Exponentialb => &[Op::Exponential],
            _ => &[],
        }
    }
}

/// `measureName`.
pub(crate) fn measure_name(op: &str, field: Option<&str>, as_: Option<&str>) -> String {
    match (as_, field) {
        (Some(as_), _) if !as_.is_empty() => as_.to_string(),
        (_, Some(field)) if !field.is_empty() => format!("{op}_{field}"),
        _ => op.to_string(),
    }
}

/// One field's measures: what `compileMeasures` makes of them.
struct Measures {
    field: Accessor,
    /// The ops whose state a value of the field updates, in `idx` order.
    ops: Vec<Op>,
    /// `exponential`'s weight.
    exponent: Option<f64>,
    /// The requested ops and their output names, in `idx` order.
    out: Vec<(Op, String)>,
}

/// An aggregation's dimensions and measures: `Aggregate.init`.
pub(crate) struct Aggregation {
    dims: Vec<Accessor>,
    /// The cell key, where `key` gives one.
    key: Option<Accessor>,
    counts: Vec<String>,
    measures: Vec<Measures>,
}

impl Aggregation {
    pub fn new(
        view: &View,
        groupby: &[Field],
        fields: &[Option<Field>],
        ops: Option<&Param<Vec<String>>>,
        params: Option<&Param<Vec<Option<f64>>>>,
        as_: Option<&Param<Vec<Option<String>>>>,
        key: Option<&Field>,
    ) -> Result<Self, Error> {
        let dims = Accessor::many(view, groupby)?;
        let key = key.map(|k| Accessor::new(view, k)).transpose()?;
        let ops = match ops {
            Some(ops) => view.param(ops)?,
            None => vec!["count".into()],
        };
        let fields: Vec<Option<Field>> = match fields.is_empty() {
            true => vec![None],
            false => fields.to_vec(),
        };
        let params = params
            .map(|p| view.param(p))
            .transpose()?
            .unwrap_or_default();
        let as_ = as_.map(|a| view.param(a)).transpose()?.unwrap_or_default();
        if fields.len() != ops.len() {
            return Err(Error::Spec(
                "Unmatched number of fields and aggregate ops.".into(),
            ));
        }
        let ops = ops
            .iter()
            .map(|op| Op::parse(op))
            .collect::<Result<Vec<_>, _>>()?;
        let fields = fields
            .iter()
            .map(|f| f.as_ref().map(|f| Accessor::new(view, f)).transpose())
            .collect::<Result<Vec<_>, _>>()?;
        Self::of(dims, key, fields, &ops, &params, &as_)
    }

    /// The measures of `ops` over `fields`, each the `i`th of its lists.
    pub fn of(
        dims: Vec<Accessor>,
        key: Option<Accessor>,
        fields: Vec<Option<Accessor>>,
        ops: &[Op],
        params: &[Option<f64>],
        as_: &[Option<String>],
    ) -> Result<Self, Error> {
        let mut counts = Vec::new();
        // Each field's requested ops, fields in the order first measured.
        let mut measures: Vec<Measures> = Vec::new();
        for (i, (field, &op)) in fields.into_iter().zip(ops).enumerate() {
            if field.is_none() && op != Op::Count {
                return Err(Error::Spec("Null aggregate field specified.".into()));
            }
            let name = field.as_ref().map(|f| f.name.clone());
            let out = measure_name(
                op.name(),
                name.as_deref(),
                as_.get(i).and_then(Option::as_deref),
            );
            if op == Op::Count {
                counts.push(out);
                continue;
            }
            let field = field.expect("a field");
            let index = match measures.iter().position(|m| m.field.name == field.name) {
                Some(index) => index,
                None => {
                    measures.push(Measures {
                        field,
                        ops: Vec::new(),
                        exponent: None,
                        out: Vec::new(),
                    });
                    measures.len() - 1
                }
            };
            let m = &mut measures[index];
            if op == Op::Exponential {
                m.exponent = params.get(i).copied().flatten();
            }
            m.out.push((op, out));
        }
        for m in &mut measures {
            // `resolve`: each op once, then what they require.
            for (op, _) in &m.out {
                if !m.ops.contains(op) {
                    m.ops.push(*op);
                }
            }
            let mut i = 0;
            while i < m.ops.len() {
                for required in m.ops[i].requires() {
                    if !m.ops.contains(required) {
                        m.ops.push(*required);
                    }
                }
                i += 1;
            }
            m.ops.sort_by_key(|op| op.index());
            m.out.sort_by_key(|(op, _)| op.index());
        }
        Ok(Self {
            dims,
            key,
            counts,
            measures,
        })
    }

    /// The tuples' cells: tuples grouped by the cell key, cells in the order
    /// their first tuple comes.
    fn cells(&self, view: &View, tuples: &[Tuple]) -> Result<Vec<Vec<usize>>, Error> {
        match &self.key {
            Some(key) => groups(view, std::slice::from_ref(key), Key::Cells, tuples),
            None => groups(view, &self.dims, Key::Cells, tuples),
        }
    }

    /// The dimension values of `t`, by their names: `newtuple`.
    fn dims(&self, view: &View, t: &Value) -> Result<Value, Error> {
        let mut dims = Value::Object(Vec::new().into());
        for dim in &self.dims {
            dims = dims.with_field(&dim.name, dim.get(view, t)?);
        }
        Ok(dims)
    }

    /// A cell with nothing in it.
    pub fn cell(&self) -> Cell<'_> {
        let store = self
            .measures
            .iter()
            .any(|m| m.ops.contains(&Op::Values))
            .then(Store::default);
        Cell {
            aggregation: self,
            num: 0.0,
            states: self.measures.iter().map(State::new).collect(),
            store,
        }
    }
}

/// An aggregation cell: its count, each field's measure state, and the
/// tuples it holds where an op reads them.
pub(crate) struct Cell<'a> {
    aggregation: &'a Aggregation,
    num: f64,
    states: Vec<State>,
    store: Option<Store>,
}

impl Cell<'_> {
    pub fn add(&mut self, view: &View, t: &Tuple) -> Result<(), Error> {
        self.num += 1.0;
        if let Some(store) = &mut self.store {
            store.add.push(t.clone());
        }
        for (state, measures) in self.states.iter_mut().zip(&self.aggregation.measures) {
            state.add(measures, measures.field.get(view, &t.datum)?, &t.datum);
        }
        Ok(())
    }

    pub fn rem(&mut self, view: &View, t: &Tuple) -> Result<(), Error> {
        self.num -= 1.0;
        if let Some(store) = &mut self.store {
            store.rem.push(t.clone());
        }
        for (state, measures) in self.states.iter_mut().zip(&self.aggregation.measures) {
            state.rem(measures, measures.field.get(view, &t.datum)?);
        }
        Ok(())
    }

    /// `celltuple`: `tuple` with the counts and each measure written.
    pub fn set(&mut self, view: &View, mut tuple: Value) -> Result<Value, Error> {
        if let Some(store) = &mut self.store {
            store.values();
        }
        for name in &self.aggregation.counts {
            tuple = tuple.with_field(name, Value::Number(self.num));
        }
        for (field, (state, measures)) in self
            .states
            .iter_mut()
            .zip(&self.aggregation.measures)
            .enumerate()
        {
            for (op, out) in &measures.out {
                let value = state.value(view, *op, measures, field, self.num, &mut self.store)?;
                tuple = tuple.with_field(out, value);
            }
        }
        Ok(tuple)
    }
}

/// One field's measure state over a cell.
struct State {
    valid: f64,
    missing: f64,
    sum: f64,
    product: f64,
    mean: f64,
    mean_d: f64,
    dev: f64,
    exp: f64,
    exponent: f64,
    min: Value,
    max: Value,
    argmin: Option<Value>,
    argmax: Option<Value>,
}

impl State {
    fn new(measures: &Measures) -> Self {
        State {
            valid: 0.0,
            missing: 0.0,
            sum: 0.0,
            product: 1.0,
            mean: 0.0,
            mean_d: 0.0,
            dev: 0.0,
            exp: 0.0,
            exponent: measures.exponent.unwrap_or(f64::NAN),
            min: Value::Undefined,
            max: Value::Undefined,
            argmin: None,
            argmax: None,
        }
    }

    fn add(&mut self, measures: &Measures, v: Value, t: &Value) {
        if v.is_nullish() || matches!(&v, Value::String(s) if s.is_empty()) {
            self.missing += 1.0;
            return;
        }
        if matches!(v, Value::Number(n) if n.is_nan()) {
            return;
        }
        self.valid += 1.0;
        let n = v.to_number();
        for op in &measures.ops {
            match op {
                Op::Sum => self.sum += n,
                Op::Product => self.product *= n,
                Op::Mean => {
                    self.mean_d = n - self.mean;
                    self.mean += self.mean_d / self.valid;
                }
                Op::Variance => self.dev += self.mean_d * (n - self.mean),
                Op::Min => {
                    if v.compare(&self.min) == Some(Ordering::Less)
                        || matches!(self.min, Value::Undefined)
                    {
                        self.min = v.clone();
                    }
                }
                Op::Max => {
                    if v.compare(&self.max) == Some(Ordering::Greater)
                        || matches!(self.max, Value::Undefined)
                    {
                        self.max = v.clone();
                    }
                }
                Op::Argmin => {
                    if v.compare(&self.min) == Some(Ordering::Less) {
                        self.argmin = Some(t.clone());
                    }
                }
                Op::Argmax => {
                    if v.compare(&self.max) == Some(Ordering::Greater) {
                        self.argmax = Some(t.clone());
                    }
                }
                Op::Exponential => self.exp = self.exponent * self.exp + n,
                _ => {}
            }
        }
    }

    fn rem(&mut self, measures: &Measures, v: Value) {
        if v.is_nullish() || matches!(&v, Value::String(s) if s.is_empty()) {
            self.missing -= 1.0;
            return;
        }
        if matches!(v, Value::Number(n) if n.is_nan()) {
            return;
        }
        self.valid -= 1.0;
        let n = v.to_number();
        let at_most =
            |bound: &Value| matches!(v.compare(bound), Some(Ordering::Less | Ordering::Equal));
        let at_least =
            |bound: &Value| matches!(v.compare(bound), Some(Ordering::Greater | Ordering::Equal));
        for op in &measures.ops {
            match op {
                Op::Sum => self.sum -= n,
                Op::Product => self.product /= n,
                Op::Mean => {
                    self.mean_d = n - self.mean;
                    self.mean -= if self.valid != 0.0 {
                        self.mean_d / self.valid
                    } else {
                        self.mean
                    };
                }
                Op::Variance => self.dev -= self.mean_d * (n - self.mean),
                Op::Min => {
                    if at_most(&self.min) {
                        self.min = Value::Number(f64::NAN);
                    }
                }
                Op::Max => {
                    if at_least(&self.max) {
                        self.max = Value::Number(f64::NAN);
                    }
                }
                Op::Argmin => {
                    if at_most(&self.min) {
                        self.argmin = None;
                    }
                }
                Op::Argmax => {
                    if at_least(&self.max) {
                        self.argmax = None;
                    }
                }
                Op::Exponential => {
                    self.exp = (self.exp - n / self.exponent.powf(self.valid - 1.0)) / self.exponent
                }
                _ => {}
            }
        }
    }

    /// An op's `value`. `field` is the index of the measures asking, for the
    /// cell's store to tell its callers apart.
    fn value(
        &mut self,
        view: &View,
        op: Op,
        measures: &Measures,
        field: usize,
        num: f64,
        store: &mut Option<Store>,
    ) -> Result<Value, Error> {
        let valid = self.valid;
        let dev = self.dev.max(0.0);
        let number = |when: bool, v: f64| match when {
            true => Value::Number(v),
            false => Value::Undefined,
        };
        let get = |t: &Value| measures.field.get(view, t);
        let nan = |v: &Value| matches!(v, Value::Number(n) if n.is_nan());
        Ok(match op {
            Op::Values => Value::from(
                held(store)
                    .values()
                    .iter()
                    .map(|t| t.datum.clone())
                    .collect::<Vec<_>>(),
            ),
            Op::Count => Value::Number(num),
            Op::CountValues => Value::Number(self.missing + valid),
            Op::Missing => Value::Number(self.missing),
            Op::Valid => Value::Number(valid),
            Op::Sum => number(valid != 0.0, self.sum),
            Op::Product => number(valid != 0.0, self.product),
            Op::Mean | Op::Average => number(valid != 0.0, self.mean),
            Op::Variance => number(valid > 1.0, dev / (valid - 1.0)),
            Op::Variancep => number(valid != 0.0, dev / valid),
            Op::Stdev => number(valid > 1.0, (dev / (valid - 1.0)).sqrt()),
            Op::Stdevp => number(valid != 0.0, (dev / valid).sqrt()),
            Op::Stderr => number(valid > 1.0, (dev / (valid * (valid - 1.0))).sqrt()),
            Op::Exponential => number(
                valid != 0.0,
                self.exp * (1.0 - self.exponent) / (1.0 - self.exponent.powf(valid)),
            ),
            Op::Exponentialb => number(valid != 0.0, self.exp * (1.0 - self.exponent)),
            Op::Min => {
                if nan(&self.min) {
                    self.min = match &held(store).extent(view, measures, field)?[0] {
                        Some(t) => get(&t.datum)?,
                        None => Value::Undefined,
                    };
                }
                self.min.clone()
            }
            Op::Max => {
                if nan(&self.max) {
                    self.max = match &held(store).extent(view, measures, field)?[1] {
                        Some(t) => get(&t.datum)?,
                        None => Value::Undefined,
                    };
                }
                self.max.clone()
            }
            Op::Argmin => match &self.argmin {
                Some(t) => t.clone(),
                None => held(store).extent(view, measures, field)?[0]
                    .as_ref()
                    .map_or(Value::Object(Vec::new().into()), |t| t.datum.clone()),
            },
            Op::Argmax => match &self.argmax {
                Some(t) => t.clone(),
                None => held(store).extent(view, measures, field)?[1]
                    .as_ref()
                    .map_or(Value::Object(Vec::new().into()), |t| t.datum.clone()),
            },
            Op::Distinct => {
                let mut seen = HashSet::new();
                for t in held(store).values() {
                    seen.insert(get(&t.datum)?.to_string());
                }
                Value::Number(seen.len() as f64)
            }
            Op::Median | Op::Q1 | Op::Q3 => {
                let store = held(store);
                if store.get != Some(field) || store.quartiles.is_none() {
                    let values = store
                        .values()
                        .iter()
                        .map(|t| get(&t.datum))
                        .collect::<Result<Vec<_>, _>>()?;
                    store.quartiles = Some(statistics::quartiles(&statistics::numbers(&values)));
                    store.get = Some(field);
                }
                let [q1, q2, q3] = store.quartiles.clone().expect("quartiles");
                match op {
                    Op::Q1 => q1,
                    Op::Q3 => q3,
                    _ => q2,
                }
            }
            Op::Ci0 | Op::Ci1 => {
                let store = held(store);
                if store.get != Some(field) || store.ci.is_none() {
                    let values = store
                        .values()
                        .iter()
                        .map(|t| get(&t.datum))
                        .collect::<Result<Vec<_>, _>>()?;
                    store.ci = Some(statistics::bootstrap_ci(
                        &statistics::numbers(&values),
                        values.len(),
                        1000,
                        0.05,
                        &mut || view.random(),
                    ));
                    store.get = Some(field);
                }
                let [low, high] = store.ci.clone().expect("ci");
                if op == Op::Ci0 { low } else { high }
            }
        })
    }
}

/// The store of a cell an op reads the tuples of; every such op's
/// measures make the cell keep one.
fn held(store: &mut Option<Store>) -> &mut Store {
    store.as_mut().expect("a store for an op that reads values")
}

/// A cell's `TupleStore`: the tuples added and removed since it last
/// consolidated them, and its statistics, memoized for the last field that
/// asked and shared by every field of the cell.
#[derive(Default)]
struct Store {
    add: Vec<Tuple>,
    rem: Vec<Tuple>,
    get: Option<usize>,
    extent: Option<[Option<Tuple>; 2]>,
    quartiles: Option<[Value; 3]>,
    ci: Option<[Value; 2]>,
}

impl Store {
    /// The tuples held: those added, each removed one taken out once.
    fn values(&mut self) -> &[Tuple] {
        self.get = None;
        if !self.rem.is_empty() {
            let mut removed: HashMap<u64, bool> =
                self.rem.drain(..).map(|t| (t.id, true)).collect();
            self.add.retain(|t| match removed.get_mut(&t.id) {
                Some(pending) if *pending => {
                    *pending = false;
                    false
                }
                _ => true,
            });
        }
        &self.add
    }

    /// vega-util's `extentIndex`: the tuples of the least and greatest
    /// value, compared as JavaScript compares them.
    fn extent(
        &mut self,
        view: &View,
        measures: &Measures,
        field: usize,
    ) -> Result<[Option<Tuple>; 2], Error> {
        if self.get != Some(field) || self.extent.is_none() {
            let mut u: Option<(usize, Value)> = None;
            let mut v: Option<(usize, Value)> = None;
            let values = self.values();
            for (i, t) in values.iter().enumerate() {
                let b = measures.field.get(view, &t.datum)?;
                if b.is_nullish() {
                    continue;
                }
                match (&u, &v) {
                    (Some((_, a)), Some((_, c))) => {
                        if a.compare(&b) == Some(Ordering::Greater) {
                            u = Some((i, b.clone()));
                        }
                        if c.compare(&b) == Some(Ordering::Less) {
                            v = Some((i, b));
                        }
                    }
                    _ => {
                        if b.compare(&b) == Some(Ordering::Equal) {
                            u = Some((i, b.clone()));
                            v = Some((i, b));
                        }
                    }
                }
            }
            let pick = |x: Option<(usize, Value)>| x.map(|(i, _)| values[i].clone());
            self.extent = Some([pick(u), pick(v)]);
            self.get = Some(field);
        }
        Ok(self.extent.clone().expect("extent"))
    }
}

/// `aggregate`: one tuple per cell, cells in the order their first tuple
/// comes.
pub fn aggregate(
    view: &View,
    def: &spec::Aggregate,
    tuples: &[Tuple],
) -> Result<Vec<Tuple>, Error> {
    let aggregation = Aggregation::new(
        view,
        &def.groupby,
        &def.fields,
        def.ops.as_ref(),
        def.aggregate_params.as_ref(),
        def.as_.as_ref(),
        def.key.as_ref(),
    )?;
    aggregation.run(view, tuples, or(view, &def.cross, false)?)
}

impl Aggregation {
    /// One tuple per cell, cells in the order their first tuple comes; with
    /// `cross` and more than one dimension, an empty cell for each missing
    /// combination of their values after them.
    pub fn run(&self, view: &View, tuples: &[Tuple], cross: bool) -> Result<Vec<Tuple>, Error> {
        let cells = self.cells(view, tuples)?;
        let mut out = Vec::with_capacity(cells.len());
        let mut rows: Vec<&[usize]> = Vec::with_capacity(cells.len());
        for cell in &cells {
            let dims = self.dims(view, &tuples[cell[0]].datum)?;
            out.push(view.ingest(dims));
            rows.push(cell);
        }
        if cross && self.dims.len() > 1 {
            let before = out.len();
            self::cross(view, self, &mut out);
            rows.extend(std::iter::repeat_n(&[][..], out.len() - before));
        }
        for (t, rows) in out.iter_mut().zip(rows) {
            let mut cell = self.cell();
            for &i in rows {
                cell.add(view, &tuples[i])?;
            }
            let dims = std::mem::take(&mut t.datum);
            t.datum = cell.set(view, dims)?;
        }
        Ok(out)
    }
}

/// `cross`: an empty cell for each combination of dimension values no cell
/// has, combinations in the order of each dimension's values as a
/// JavaScript object's keys order them.
fn cross(view: &View, aggregation: &Aggregation, out: &mut Vec<Tuple>) {
    let names: Vec<&str> = aggregation.dims.iter().map(|d| d.name.as_str()).collect();
    let mut values: Vec<Vec<(String, Value)>> = vec![Vec::new(); names.len()];
    let mut keys: HashSet<String> = HashSet::new();
    for t in out.iter() {
        let mut key = Vec::new();
        for (i, name) in names.iter().enumerate() {
            let v = t.datum.get(&(*name).into()).unwrap_or_default();
            let text = v.to_string();
            match values[i].iter_mut().find(|(k, _)| *k == text) {
                Some((_, slot)) => *slot = v,
                None => values[i].push((text.clone(), v)),
            }
            key.push(text);
        }
        keys.insert(key.join("|"));
    }
    for values in &mut values {
        // Integer keys first, ascending, then the rest as inserted.
        let index = |k: &str| {
            k.parse::<u32>()
                .ok()
                .filter(|i| i.to_string() == k && *i != u32::MAX)
        };
        values.sort_by(|(a, _), (b, _)| match (index(a), index(b)) {
            (Some(a), Some(b)) => a.cmp(&b),
            (Some(_), None) => Ordering::Less,
            (None, Some(_)) => Ordering::Greater,
            (None, None) => Ordering::Equal,
        });
    }
    if values.iter().any(Vec::is_empty) {
        return;
    }
    let mut combination = vec![0; names.len()];
    'outer: loop {
        let key: Vec<&str> = combination
            .iter()
            .enumerate()
            .map(|(i, &j)| values[i][j].0.as_str())
            .collect();
        if !keys.contains(&key.join("|")) {
            let mut dims = Value::Object(Vec::new().into());
            for (i, &j) in combination.iter().enumerate() {
                dims = dims.with_field(names[i], values[i][j].1.clone());
            }
            out.push(view.ingest(dims));
        }
        for i in (0..names.len()).rev() {
            combination[i] += 1;
            if combination[i] < values[i].len() {
                continue 'outer;
            }
            combination[i] = 0;
        }
        break;
    }
}

/// `joinaggregate`: each tuple with its cell's counts and measures.
pub fn join(
    view: &View,
    def: &spec::Joinaggregate,
    mut tuples: Vec<Tuple>,
) -> Result<Vec<Tuple>, Error> {
    let aggregation = Aggregation::new(
        view,
        &def.groupby,
        &def.fields,
        def.ops.as_ref(),
        None,
        def.as_.as_ref(),
        def.key.as_ref(),
    )?;
    let cells = aggregation.cells(view, &tuples)?;
    let mut joined = Vec::with_capacity(cells.len());
    for rows in &cells {
        let mut cell = aggregation.cell();
        for &i in rows {
            cell.add(view, &tuples[i])?;
        }
        let dims = aggregation.dims(view, &tuples[rows[0]].datum)?;
        joined.push(cell.set(view, dims)?);
    }
    for (cell, rows) in joined.iter().zip(&cells) {
        let Value::Object(fields) = cell else {
            continue;
        };
        for &i in rows {
            for (k, v) in fields.iter() {
                tuples[i].datum = tuples[i].datum.with_field(k, v.clone());
            }
        }
    }
    Ok(tuples)
}
