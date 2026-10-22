//! Vega-Lite's `regression` transform: least-squares trend lines.

use crate::{
    data::{Column, Data},
    transform::Groups,
};
use gpui::SharedString;
use vega::{
    statistics::{
        curve,
        regression::{Fit, Method},
    },
    value::Value,
};

/// A fit of `field` on `on` per group, sampled along `on`: one row per
/// sample, or with `params` one per group. A group with no more rows than
/// the fit has parameters is left out.
#[derive(Clone, Debug, PartialEq)]
pub struct Regression {
    /// The predicted field.
    pub field: SharedString,
    /// The predictor field.
    pub on: SharedString,
    pub groupby: Vec<SharedString>,
    pub method: Method,
    /// Where to sample. `None` takes the extent of `on`. A log fit ignores
    /// one that starts at or below 0.
    pub extent: Option<[f64; 2]>,
    /// The predictor's and the prediction's fields, Vega-Lite's `as`.
    pub output: [SharedString; 2],
    /// Each group's fit in place of its samples, as Vega gives it: `keys`,
    /// the group's values in `groupby` order, undefined without `groupby`;
    /// `coef`, the coefficients, the constant's first; and `rSquared`.
    pub params: bool,
}

impl Regression {
    /// Vega-Lite's defaults: linear, output as `on` and `field`.
    pub fn new(field: impl Into<SharedString>, on: impl Into<SharedString>) -> Self {
        let (field, on) = (field.into(), on.into());
        Self {
            output: [on.clone(), field.clone()],
            field,
            on,
            groupby: Vec::new(),
            method: Method::Linear,
            extent: None,
            params: false,
        }
    }

    pub fn apply(&self, data: &Data) -> Data {
        let xs = data.numbers(&self.on);
        let ys = data.numbers(&self.field);
        let groups = Groups::of(data, &self.groupby);
        let domain = self
            .extent
            .filter(|[low, _]| !(self.method == Method::Log && *low <= 0.0));
        let parameters = match self.method {
            Method::Poly(order) => order,
            Method::Quad => 2,
            _ => 1,
        };
        let fits = groups.rows.iter().map(|rows| {
            let x: Vec<f64> = rows.iter().map(|row| xs[*row]).collect();
            let y: Vec<f64> = rows.iter().map(|row| ys[*row]).collect();
            let fit = (rows.len() > parameters).then(|| Fit::new(self.method, &x, &y));
            (fit, x, y)
        });
        if self.params {
            let owners: Vec<usize> = groups.rows.iter().map(|rows| rows[0]).collect();
            let keys = groups.keys(data, &owners);
            let (mut dims, mut coef, mut r_squared) = (Vec::new(), Vec::new(), Vec::new());
            for (group, (fit, x, y)) in fits.enumerate() {
                let Some(fit) = fit else {
                    continue;
                };
                dims.push(match self.groupby.is_empty() {
                    true => Value::Undefined,
                    false => Value::Array(
                        keys.fields()
                            .map(|(_, column)| column.value(group))
                            .collect(),
                    ),
                });
                coef.push(Value::Array(
                    fit.coef().into_iter().map(Value::Number).collect(),
                ));
                r_squared.push(fit.r_squared(&x, &y));
            }
            return Data::new()
                .with("keys", Column::Value(dims.into()))
                .with("coef", Column::Value(coef.into()))
                .number("rSquared", r_squared);
        }
        let points: Vec<Vec<[f64; 2]>> = fits
            .map(|(fit, x, _)| {
                let Some(fit) = fit else {
                    return Vec::new();
                };
                let Some(domain) = domain.or_else(|| curve::extent(x.iter().copied())) else {
                    return Vec::new();
                };
                match self.method {
                    Method::Linear => domain.map(|x| [x, fit.predict(x)]).to_vec(),
                    _ => curve::sample(|x| fit.predict(x), domain, 25, 200),
                }
            })
            .collect();
        groups.table(data, &points, &self.output)
    }
}
