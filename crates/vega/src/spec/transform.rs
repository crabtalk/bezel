use crate::spec::{Many, SignalRef};
use serde::{Deserialize, Serialize};
use serde_json::Value as Json;

/// A data transform, by its `type`. `signal` on a transform names a signal
/// that holds the transform's value.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Transform {
    Aggregate(Aggregate),
    Bin(Bin),
    Collect(Collect),
    Countpattern(Countpattern),
    Cross(Cross),
    Density(Density),
    Dotbin(Dotbin),
    Extent(Extent),
    Filter(Filter),
    Flatten(Flatten),
    Fold(Fold),
    Formula(Formula),
    Identifier(Identifier),
    Impute(Impute),
    Joinaggregate(Joinaggregate),
    Kde(Kde),
    Loess(Loess),
    Lookup(Lookup),
    Pivot(Pivot),
    Project(Project),
    Quantile(Quantile),
    Regression(Regression),
    Sample(Sample),
    Sequence(Sequence),
    Stack(Stack),
    Timeunit(Timeunit),
    Window(Window),
}

/// A parameter's value as written, or a signal that holds it.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(untagged)]
pub enum Param<T> {
    Signal(SignalRef),
    Value(T),
}

/// A field parameter: a field name, `{field, as}`, an expression over the
/// datum `{expr, as}`, or a signal that holds a field name.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(untagged)]
pub enum Field {
    Name(String),
    Signal(SignalRef),
    Field {
        field: String,
        #[serde(rename = "as", skip_serializing_if = "Option::is_none")]
        as_: Option<String>,
    },
    Expr {
        expr: String,
        #[serde(rename = "as", skip_serializing_if = "Option::is_none")]
        as_: Option<String>,
    },
}

/// A comparator: fields compared in turn, each in its order.
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Compare {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub field: Option<Many<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub order: Option<Many<Order>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Order {
    Ascending,
    Descending,
}

#[derive(Clone, Debug, Default, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Aggregate {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub groupby: Vec<Field>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ops: Option<Param<Vec<String>>>,
    #[serde(rename = "aggregate_params", skip_serializing_if = "Option::is_none")]
    pub aggregate_params: Option<Param<Vec<Option<f64>>>>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fields: Vec<Option<Field>>,
    #[serde(rename = "as", skip_serializing_if = "Option::is_none")]
    pub as_: Option<Param<Vec<Option<String>>>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub drop: Option<Param<bool>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cross: Option<Param<bool>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key: Option<Field>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signal: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Bin {
    pub field: Field,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub interval: Option<Param<bool>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub anchor: Option<Param<f64>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub maxbins: Option<Param<f64>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base: Option<Param<f64>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub divide: Option<Param<Vec<f64>>>,
    pub extent: Param<Vec<Json>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub span: Option<Param<f64>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub step: Option<Param<f64>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub steps: Option<Param<Vec<f64>>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub minstep: Option<Param<f64>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nice: Option<Param<bool>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<Param<String>>,
    #[serde(rename = "as", skip_serializing_if = "Option::is_none")]
    pub as_: Option<Param<[String; 2]>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signal: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Collect {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sort: Option<Compare>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signal: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Countpattern {
    pub field: Field,
    #[serde(rename = "case", skip_serializing_if = "Option::is_none")]
    pub case: Option<Param<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pattern: Option<Param<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stopwords: Option<Param<String>>,
    #[serde(rename = "as", skip_serializing_if = "Option::is_none")]
    pub as_: Option<Param<[String; 2]>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signal: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Cross {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub filter: Option<String>,
    #[serde(rename = "as", skip_serializing_if = "Option::is_none")]
    pub as_: Option<Param<[String; 2]>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signal: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Density {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extent: Option<Param<[f64; 2]>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub steps: Option<Param<f64>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub minsteps: Option<Param<f64>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub maxsteps: Option<Param<f64>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub method: Option<Param<String>>,
    pub distribution: Distribution,
    #[serde(rename = "as", skip_serializing_if = "Option::is_none")]
    pub as_: Option<Param<Vec<String>>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signal: Option<String>,
}

/// A probability distribution, by its `function`.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(tag = "function", rename_all = "lowercase", deny_unknown_fields)]
pub enum Distribution {
    Normal {
        #[serde(skip_serializing_if = "Option::is_none")]
        mean: Option<Param<f64>>,
        #[serde(skip_serializing_if = "Option::is_none")]
        stdev: Option<Param<f64>>,
    },
    Lognormal {
        #[serde(skip_serializing_if = "Option::is_none")]
        mean: Option<Param<f64>>,
        #[serde(skip_serializing_if = "Option::is_none")]
        stdev: Option<Param<f64>>,
    },
    Uniform {
        #[serde(skip_serializing_if = "Option::is_none")]
        min: Option<Param<f64>>,
        #[serde(skip_serializing_if = "Option::is_none")]
        max: Option<Param<f64>>,
    },
    Kde {
        field: Field,
        #[serde(skip_serializing_if = "Option::is_none")]
        from: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        bandwidth: Option<Param<f64>>,
    },
    Mixture {
        distributions: Vec<Distribution>,
        #[serde(skip_serializing_if = "Option::is_none")]
        weights: Option<Param<Vec<f64>>>,
    },
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Dotbin {
    pub field: Field,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub groupby: Vec<Field>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub step: Option<Param<f64>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub smooth: Option<Param<bool>>,
    #[serde(rename = "as", skip_serializing_if = "Option::is_none")]
    pub as_: Option<Param<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signal: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Extent {
    pub field: Field,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signal: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Filter {
    pub expr: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signal: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Flatten {
    pub fields: Vec<Field>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub index: Option<Param<String>>,
    #[serde(rename = "as", skip_serializing_if = "Option::is_none")]
    pub as_: Option<Param<Vec<String>>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signal: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Fold {
    pub fields: Vec<Field>,
    #[serde(rename = "as", skip_serializing_if = "Option::is_none")]
    pub as_: Option<Param<[String; 2]>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signal: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Formula {
    pub expr: String,
    #[serde(rename = "as")]
    pub as_: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub initonly: Option<Param<bool>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signal: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Identifier {
    #[serde(rename = "as")]
    pub as_: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signal: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Impute {
    pub field: Field,
    pub key: Field,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub keyvals: Option<Param<Vec<Json>>>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub groupby: Vec<Field>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub method: Option<Param<String>>,
    /// `Some(Null)` where `value` is written as `null`.
    #[serde(
        default,
        deserialize_with = "crate::spec::present",
        skip_serializing_if = "Option::is_none"
    )]
    pub value: Option<Param<Json>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signal: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Joinaggregate {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub groupby: Vec<Field>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fields: Vec<Option<Field>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ops: Option<Param<Vec<String>>>,
    #[serde(rename = "as", skip_serializing_if = "Option::is_none")]
    pub as_: Option<Param<Vec<Option<String>>>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key: Option<Field>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signal: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Kde {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub groupby: Vec<Field>,
    pub field: Field,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cumulative: Option<Param<bool>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub counts: Option<Param<bool>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bandwidth: Option<Param<f64>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extent: Option<Param<[f64; 2]>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resolve: Option<Param<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub steps: Option<Param<f64>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub minsteps: Option<Param<f64>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub maxsteps: Option<Param<f64>>,
    #[serde(rename = "as", skip_serializing_if = "Option::is_none")]
    pub as_: Option<Param<Vec<String>>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signal: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Loess {
    pub x: Field,
    pub y: Field,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub groupby: Vec<Field>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bandwidth: Option<Param<f64>>,
    #[serde(rename = "as", skip_serializing_if = "Option::is_none")]
    pub as_: Option<Param<Vec<String>>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signal: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Lookup {
    pub from: String,
    pub key: Field,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub values: Vec<Field>,
    pub fields: Vec<Field>,
    #[serde(rename = "as", skip_serializing_if = "Option::is_none")]
    pub as_: Option<Param<Vec<String>>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default: Option<Param<Json>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signal: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Pivot {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub groupby: Vec<Field>,
    pub field: Field,
    pub value: Field,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub op: Option<Param<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<Param<f64>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key: Option<Field>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signal: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Project {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fields: Vec<Field>,
    #[serde(rename = "as", skip_serializing_if = "Option::is_none")]
    pub as_: Option<Param<Vec<Option<String>>>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signal: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Quantile {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub groupby: Vec<Field>,
    pub field: Field,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub probs: Option<Param<Vec<f64>>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub step: Option<Param<f64>>,
    #[serde(rename = "as", skip_serializing_if = "Option::is_none")]
    pub as_: Option<Param<[String; 2]>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signal: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Regression {
    pub x: Field,
    pub y: Field,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub groupby: Vec<Field>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub method: Option<Param<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub order: Option<Param<f64>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extent: Option<Param<[f64; 2]>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<Param<bool>>,
    #[serde(rename = "as", skip_serializing_if = "Option::is_none")]
    pub as_: Option<Param<Vec<String>>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signal: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Sample {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<Param<f64>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signal: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Sequence {
    pub start: Param<f64>,
    pub stop: Param<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub step: Option<Param<f64>>,
    #[serde(rename = "as", skip_serializing_if = "Option::is_none")]
    pub as_: Option<Param<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signal: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Stack {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub field: Option<Field>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub groupby: Vec<Field>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sort: Option<Compare>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub offset: Option<Param<Offset>>,
    #[serde(rename = "as", skip_serializing_if = "Option::is_none")]
    pub as_: Option<Param<[String; 2]>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signal: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Offset {
    Zero,
    Center,
    Normalize,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Timeunit {
    pub field: Field,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub interval: Option<Param<bool>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub units: Option<Param<Vec<String>>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub step: Option<Param<f64>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub maxbins: Option<Param<f64>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extent: Option<Param<Vec<Json>>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub infer_units: Option<Param<bool>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timezone: Option<Param<String>>,
    #[serde(rename = "as", skip_serializing_if = "Option::is_none")]
    pub as_: Option<Param<[String; 2]>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signal: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Window {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sort: Option<Compare>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub groupby: Vec<Field>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ops: Option<Param<Vec<String>>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<Param<Vec<Option<f64>>>>,
    #[serde(rename = "aggregate_params", skip_serializing_if = "Option::is_none")]
    pub aggregate_params: Option<Param<Vec<Option<f64>>>>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fields: Vec<Option<Field>>,
    #[serde(rename = "as", skip_serializing_if = "Option::is_none")]
    pub as_: Option<Param<Vec<Option<String>>>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frame: Option<Param<[Option<f64>; 2]>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ignore_peers: Option<Param<bool>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signal: Option<String>,
}
