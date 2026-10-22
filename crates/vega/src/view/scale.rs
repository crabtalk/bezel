//! A scale definition resolved for vega-encode's `Scale` transform, as
//! vega-parser's `parseScale` wires it: its domain read from data, its
//! range from the config's named ranges, and its signals.

use crate::{
    Error,
    scale::{self, NiceParam, Params, Scale, SchemeParam},
    spec::{
        self, Bins, Compare, DataDomain, Domain, DomainField, Field, Interpolate, Many, Nice,
        Order, Param, Range, Sort,
    },
    transform::{self, Accessor, Comparator, Tuple, aggregate},
    value::Value,
    view::View,
};
use serde_json::Value as Json;

/// A domain's field: `field` of the data set `data`, or literal values.
enum Source {
    Field { data: String, field: String },
    Values(Vec<Value>),
}

/// A discrete domain's order: by `field` of its counts, after `op`.
struct SortBy {
    op: Option<String>,
    field: Option<String>,
    descending: bool,
}

impl View<'_> {
    pub(super) fn build_scale(&self, def: &spec::Scale) -> Result<Scale, Error> {
        let type_: scale::Type = def
            .type_
            .as_deref()
            .unwrap_or("linear")
            .parse()
            .map_err(Error::Spec)?;
        let value = |v: &Option<Json>| -> Result<Option<Value>, Error> {
            v.as_ref().map(|v| self.property(v)).transpose()
        };
        // A property `!= null`: neither null nor undefined.
        let present = |v: &Option<Json>| -> Result<Option<Value>, Error> {
            Ok(value(v)?.filter(|v| !v.is_nullish()))
        };
        let number = |v: &Option<Json>| -> Result<Option<f64>, Error> {
            Ok(present(v)?.map(|v| v.to_number()))
        };
        let flag =
            |v: &Option<Json>| -> Result<Option<bool>, Error> { Ok(value(v)?.map(|v| v.truthy())) };
        let mut params = Params {
            padding: number(&def.padding)?,
            padding_inner: number(&def.padding_inner)?,
            padding_outer: number(&def.padding_outer)?,
            align: number(&def.align)?,
            round: flag(&def.round)?.unwrap_or(false),
            reverse: flag(&def.reverse)?.unwrap_or(false),
            clamp: flag(&def.clamp)?.unwrap_or(false),
            zero: value(&def.zero)?
                .filter(|v| !matches!(v, Value::Undefined))
                .map(|v| v.truthy()),
            base: number(&def.base)?,
            exponent: number(&def.exponent)?,
            constant: number(&def.constant)?,
            domain_min: present(&def.domain_min)?,
            domain_max: present(&def.domain_max)?,
            domain_mid: present(&def.domain_mid)?,
            domain_implicit: flag(&def.domain_implicit)?.unwrap_or(false),
            ..Params::default()
        };
        params.domain_raw = match value(&def.domain_raw)? {
            Some(Value::Array(items)) => Some(items.to_vec()),
            _ => None,
        };
        params.domain = match &def.domain {
            None => {
                if params.domain_min.is_some() || params.domain_max.is_some() {
                    return Err(Error::Spec(
                        "No scale domain defined for domainMin/domainMax to override.".into(),
                    ));
                }
                None
            }
            Some(domain) => Some(self.domain(type_, domain)?),
        };
        if let Some(range) = &def.range {
            self.range(type_, range, &mut params)?;
        }
        match &def.interpolate {
            None => {}
            Some(Interpolate::Name(name)) => params.interpolate = Some(name.clone()),
            Some(Interpolate::Signal(signal)) => {
                params.interpolate = Some(self.signal_ref(signal)?.to_string())
            }
            Some(Interpolate::Method { type_, gamma }) => {
                params.interpolate = Some(self.property(type_)?.to_string());
                params.interpolate_gamma = gamma
                    .as_ref()
                    .map(|g| self.property(g).map(|g| g.to_number()))
                    .transpose()?;
            }
        }
        params.nice = match &def.nice {
            None => None,
            Some(Nice::Flag(false)) => None,
            Some(Nice::Flag(true)) => Some(NiceParam::Default),
            Some(Nice::Count(n)) => Some(NiceParam::Count(*n)),
            Some(Nice::Unit(unit)) => Some(NiceParam::Interval(unit.clone(), 1.0)),
            Some(Nice::Interval(interval)) => Some(NiceParam::Interval(
                self.property(&interval.interval)?.to_string(),
                interval
                    .step
                    .as_ref()
                    .map(|s| self.property(s).map(|s| s.to_number()))
                    .transpose()?
                    .unwrap_or(1.0),
            )),
            Some(Nice::Signal(signal)) => match self.signal_ref(signal)? {
                Value::Bool(true) => Some(NiceParam::Default),
                Value::String(unit) => Some(NiceParam::Interval(unit.to_string(), 1.0)),
                v if !v.truthy() => None,
                v => Some(NiceParam::Count(v.to_number())),
            },
        };
        match &def.bins {
            None => {}
            Some(Bins::Values(values)) => {
                params.bins = Some(
                    values
                        .iter()
                        .map(|v| self.property(v).map(|v| v.to_number()))
                        .collect::<Result<_, _>>()?,
                )
            }
            Some(Bins::Signal(signal)) => match self.signal_ref(signal)? {
                Value::Array(items) => {
                    params.bins = Some(items.iter().map(Value::to_number).collect())
                }
                v if v.is_object() => {
                    let get = |k: &str| v.get(&k.into()).unwrap_or_default();
                    let bound = |k: &str| match get(k) {
                        v if v.is_nullish() => f64::NAN,
                        v => v.to_number(),
                    };
                    params.bins_step =
                        Some([get("step").to_number(), bound("start"), bound("stop")]);
                }
                _ => {}
            },
            Some(Bins::Step { step, start, stop }) => {
                let bound = |v: &Option<Json>| -> Result<f64, Error> {
                    Ok(present(v)?.map_or(f64::NAN, |v| v.to_number()))
                };
                params.bins_step = Some([
                    self.property(step)?.to_number(),
                    bound(start)?,
                    bound(stop)?,
                ]);
            }
        }
        Scale::new(type_, &params)
    }

    /// `parseScaleDomain`.
    fn domain(&self, type_: scale::Type, domain: &Domain) -> Result<Vec<Value>, Error> {
        match domain {
            Domain::Signal(signal) => Ok(match self.signal_ref(signal)? {
                Value::Array(items) => items.to_vec(),
                other => vec![other],
            }),
            Domain::Values(values) => values.iter().map(|v| self.property(v)).collect(),
            Domain::Data(domain) => self.data_domain(type_, domain),
        }
    }

    /// A domain read from data: a discrete scale's distinct values, a
    /// quantile scale's every value, a continuous scale's extent.
    fn data_domain(&self, type_: scale::Type, domain: &DataDomain) -> Result<Vec<Value>, Error> {
        let sort = match &domain.sort {
            None | Some(Sort::Flag(false)) => None,
            Some(Sort::Flag(true)) => Some(SortBy {
                op: None,
                field: Some("key".into()),
                descending: false,
            }),
            Some(Sort::By(by)) => {
                let field = match (&by.field, &by.op) {
                    (None, None) => Some("key".into()),
                    (None, Some(op)) if op != "count" => {
                        return Err(Error::Spec(format!(
                            "No field provided for sort aggregate op: {op}"
                        )));
                    }
                    (field, _) => field.clone(),
                };
                Some(SortBy {
                    op: by.op.clone(),
                    field,
                    descending: by.order.as_deref() == Some("descending"),
                })
            }
        };
        let Some(fields) = &domain.fields else {
            let data = domain
                .data
                .as_deref()
                .ok_or_else(|| Error::Spec("Can not find data set: undefined".into()))?;
            let field = domain.field.as_deref().unwrap_or_default();
            let tuples = self.data(data)?;
            return if type_.is_discrete() {
                let counts = self.counts(tuples, field, sort.as_ref())?;
                self.sorted_keys(counts, sort.as_ref())
            } else if type_.is_quantile() {
                let accessor = Accessor::field(field)?;
                tuples
                    .iter()
                    .map(|t| accessor.get(self, &t.datum))
                    .collect()
            } else {
                Ok(transform::extent(self, &Accessor::field(field)?, tuples)?.to_vec())
            };
        };
        let sources = fields
            .iter()
            .map(|f| -> Result<Source, Error> {
                Ok(match f {
                    DomainField::Name(field) => Source::Field {
                        data: domain.data.clone().unwrap_or_default(),
                        field: field.clone(),
                    },
                    DomainField::Ref { data, field } => Source::Field {
                        data: data.clone(),
                        field: field.clone(),
                    },
                    DomainField::Values(values) => Source::Values(
                        values
                            .iter()
                            .map(|v| self.property(v))
                            .collect::<Result<_, _>>()?,
                    ),
                    DomainField::Signal(signal) => {
                        Source::Values(match self.signal_ref(signal)? {
                            Value::Array(items) => items.to_vec(),
                            other => vec![other],
                        })
                    }
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        // Each source as tuples, and the field to read of them.
        let sets = sources
            .into_iter()
            .map(|source| -> Result<(Vec<Tuple>, String), Error> {
                Ok(match source {
                    Source::Field { data, field } => (self.data(&data)?.to_vec(), field),
                    Source::Values(values) => (
                        values
                            .into_iter()
                            .map(|v| self.ingest(Value::Object(vec![("data".into(), v)].into())))
                            .collect(),
                        "data".into(),
                    ),
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        if type_.is_discrete() {
            if let Some(SortBy {
                op: Some(op),
                field: Some(_),
                ..
            }) = &sort
                && !matches!(op.as_str(), "min" | "max" | "count")
            {
                return Err(Error::Spec(format!(
                    "Multiple domain scales can not be sorted using {op}"
                )));
            }
            // Each source's counts, then their sum (or least, or greatest)
            // over all, by key.
            let mut counts = Vec::new();
            for (tuples, field) in &sets {
                counts.extend(self.counts(tuples, field, sort.as_ref())?);
            }
            let (op, field) = match &sort {
                Some(sort) => {
                    let a = sort.op.clone().unwrap_or("count".into());
                    let v = match &sort.field {
                        Some(f) => aggregate_field(Some(&a), Some(f)),
                        None => "count".to_string(),
                    };
                    let op = match a.as_str() {
                        "count" => "sum".to_string(),
                        other => other.to_string(),
                    };
                    (op, v)
                }
                None => ("count".to_string(), String::new()),
            };
            let def = spec::Aggregate {
                groupby: vec![Field::Name("key".into())],
                ops: Some(Param::Value(vec![op.clone()])),
                fields: match op.as_str() {
                    "count" if sort.is_none() => vec![None],
                    _ => vec![Some(Field::Name(field.clone()))],
                },
                as_: Some(Param::Value(vec![Some(match sort {
                    Some(_) => field.clone(),
                    None => "count".into(),
                })])),
                ..spec::Aggregate::default()
            };
            let merged = aggregate::aggregate(self, &def, &counts)?;
            self.sorted_keys(merged, sort.as_ref())
        } else if type_.is_quantile() {
            let mut values = Vec::new();
            for (tuples, field) in &sets {
                let accessor = Accessor::field(field)?;
                for t in tuples {
                    values.push(accessor.get(self, &t.datum)?);
                }
            }
            Ok(values)
        } else {
            // `MultiExtent`.
            let (mut min, mut max) = (
                Value::Number(f64::INFINITY),
                Value::Number(f64::NEG_INFINITY),
            );
            for (tuples, field) in &sets {
                let [lo, hi] = transform::extent(self, &Accessor::field(field)?, tuples)?;
                if lo.compare(&min) == Some(std::cmp::Ordering::Less) {
                    min = lo;
                }
                if hi.compare(&max) == Some(std::cmp::Ordering::Greater) {
                    max = hi;
                }
            }
            Ok(vec![min, max])
        }
    }

    /// `countsRef`: one tuple per distinct value of `field`, as `key`, with
    /// its `count`, and the sort's aggregate where it sorts by a field.
    fn counts(
        &self,
        tuples: &[Tuple],
        field: &str,
        sort: Option<&SortBy>,
    ) -> Result<Vec<Tuple>, Error> {
        let mut ops = vec!["count".to_string()];
        let mut fields = vec![None];
        let mut as_ = vec![Some("count".to_string())];
        if let Some(SortBy {
            op: Some(op),
            field: Some(f),
            ..
        }) = sort
        {
            ops.push(op.clone());
            fields.push(Some(Field::Name(f.clone())));
            as_.push(Some(aggregate_field(Some(op), Some(f))));
        }
        let def = spec::Aggregate {
            groupby: vec![Field::Field {
                field: field.to_string(),
                as_: Some("key".into()),
            }],
            ops: Some(Param::Value(ops)),
            fields,
            as_: Some(Param::Value(as_)),
            ..spec::Aggregate::default()
        };
        aggregate::aggregate(self, &def, tuples)
    }

    /// `Values`: the counts' keys, sorted where the domain sorts.
    fn sorted_keys(
        &self,
        mut counts: Vec<Tuple>,
        sort: Option<&SortBy>,
    ) -> Result<Vec<Value>, Error> {
        if let Some(sort) = sort {
            let compare = Compare {
                field: Some(Many::One(aggregate_field(
                    sort.op.as_deref(),
                    sort.field.as_deref(),
                ))),
                order: Some(Many::One(match sort.descending {
                    true => Order::Descending,
                    false => Order::Ascending,
                })),
            };
            if let Some(comparator) = Comparator::new(&compare)? {
                comparator.sort(self, &mut counts)?;
            }
        }
        let key = Accessor::field("key")?;
        counts.iter().map(|t| key.get(self, &t.datum)).collect()
    }

    /// `parseScaleRange`.
    fn range(&self, type_: scale::Type, range: &Range, params: &mut Params) -> Result<(), Error> {
        let values = |items: &[Json]| -> Result<Vec<Value>, Error> {
            items.iter().map(|v| self.property(v)).collect()
        };
        match range {
            Range::Signal(signal) => {
                params.range = Some(match self.signal_ref(signal)? {
                    Value::Array(items) => items.to_vec(),
                    other => vec![other],
                })
            }
            Range::Named(name) => {
                if let Some(config) = self.config.group("range").get(name) {
                    let range: Range = serde_json::from_value(config.clone())
                        .map_err(|e| Error::Spec(e.to_string()))?;
                    return self.range(type_, &range, params);
                }
                let signal = |name: &str| Value::Number(self.signal(name).to_number());
                params.range = Some(match name.as_str() {
                    "width" => vec![Value::Number(0.0), signal("width")],
                    "height" if type_.is_discrete() => vec![Value::Number(0.0), signal("height")],
                    "height" => vec![signal("height"), Value::Number(0.0)],
                    other => {
                        return Err(Error::Spec(format!(
                            "Unrecognized scale range value: \"{other}\""
                        )));
                    }
                });
            }
            Range::Step(step) => params.range_step = Some(self.property(&step.step)?.to_number()),
            Range::Scheme(scheme) => {
                params.scheme = Some(match self.property(&scheme.scheme)? {
                    Value::Array(items) => SchemeParam::Colors(items.to_vec()),
                    other => SchemeParam::Named(other.to_string()),
                });
                if let Some(extent) = &scheme.extent {
                    let extent = match self.property(extent)? {
                        Value::Array(items) => {
                            items.iter().map(Value::to_number).collect::<Vec<_>>()
                        }
                        _ => Vec::new(),
                    };
                    if extent.len() >= 2 {
                        params.scheme_extent = Some([extent[0], extent[extent.len() - 1]]);
                    }
                }
                params.scheme_count = scheme
                    .count
                    .as_ref()
                    .map(|c| self.property(c).map(|c| c.to_number()))
                    .transpose()?;
            }
            Range::Values(items) => params.range = Some(values(items)?),
            Range::Data(domain) if type_.is_discrete() => {
                params.range = Some(self.data_domain(type_, domain)?)
            }
            Range::Data(_) => {
                return Err(Error::Spec("Unsupported range type: object".into()));
            }
        }
        Ok(())
    }
}

/// `aggrField`: the name an aggregate of `field` by `op` is written as, the
/// field's runs of non-word characters as `_`.
fn aggregate_field(op: Option<&str>, field: Option<&str>) -> String {
    let op = op.unwrap_or_default();
    let field = field.unwrap_or_default();
    let mut cleaned = String::new();
    let mut gap = false;
    for c in field.chars() {
        if c.is_ascii_alphanumeric() || c == '_' {
            if gap && !cleaned.is_empty() {
                cleaned.push('_');
            }
            gap = false;
            cleaned.push(c);
        } else {
            gap = true;
        }
    }
    let cleaned = cleaned.trim_matches('_');
    let sep = if !op.is_empty() && !field.is_empty() {
        "_"
    } else {
        ""
    };
    format!("{op}{sep}{cleaned}")
}
