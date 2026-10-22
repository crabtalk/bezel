//! Vega-Lite's `regression` transform: least-squares trend lines.

use crate::{
    data::{Column, Data},
    transform::{Groups, curve},
    value::Value,
};
use d3::format::number;
use gpui::SharedString;

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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Method {
    /// `y = a + b·x`, sampled at the extent's two ends.
    Linear,
    /// `y = a + b·ln(x)`
    Log,
    /// `y = a·e^(b·x)`
    Exp,
    /// `y = a·x^b`
    Pow,
    /// `y = a + b·x + c·x²`
    Quad,
    /// A polynomial of this order.
    Poly(usize),
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

/// A fitted curve, as Vega's `regression*` functions compute it.
enum Fit {
    Constant(f64),
    Linear([f64; 2]),
    Log([f64; 2]),
    /// `e^(c0 + c1·(x - mean x))`
    Exp {
        coef: [f64; 2],
        ux: f64,
    },
    /// `c0·x^c1`
    Pow([f64; 2]),
    /// `a·x'² + b·x' + c + uy` for `x' = x - ux`.
    Quad {
        abc: [f64; 3],
        ux: f64,
        uy: f64,
    },
    /// Coefficients over `x - ux`, lowest order first, plus `uy`.
    Poly {
        coef: Vec<f64>,
        ux: f64,
        uy: f64,
    },
}

impl Fit {
    fn new(method: Method, x: &[f64], y: &[f64]) -> Self {
        match method {
            Method::Linear | Method::Poly(1) => Self::Linear(Self::means(x, y, |x| x, |y| y)),
            Method::Log => Self::Log(Self::means(x, y, f64::ln, |y| y)),
            Method::Exp => Self::exp(x, y),
            Method::Pow => {
                let [a, b] = Self::means(x, y, f64::ln, f64::ln);
                Self::Pow([a.exp(), b])
            }
            Method::Quad | Method::Poly(2) => Self::quad(x, y),
            Method::Poly(0) => Self::constant(x, y),
            Method::Poly(order) => Self::poly(x, y, order),
        }
    }

    fn predict(&self, x: f64) -> f64 {
        match self {
            Self::Constant(mean) => *mean,
            Self::Linear([a, b]) => a + b * x,
            Self::Log([a, b]) => a + b * x.ln(),
            Self::Exp { coef: [a, b], ux } => (a + b * (x - ux)).exp(),
            Self::Pow([a, b]) => a * x.powf(*b),
            Self::Quad {
                abc: [a, b, c],
                ux,
                uy,
            } => {
                let x = x - ux;
                a * x * x + b * x + c + uy
            }
            Self::Poly { coef, ux, uy } => {
                let x = x - ux;
                let mut y = uy + coef[0] + coef[1] * x + coef[2] * x * x;
                for (i, c) in coef.iter().enumerate().skip(3) {
                    y += c * x.powf(i as f64);
                }
                y
            }
        }
    }

    /// Vega's `coef`: the coefficients of `x`'s powers, lowest first, but an
    /// exponential's `[a, b]` of `a·e^(b·x)` and a power's of `a·x^b`.
    fn coef(&self) -> Vec<f64> {
        match self {
            Self::Constant(mean) => vec![*mean],
            Self::Linear(coef) | Self::Log(coef) | Self::Pow(coef) => coef.to_vec(),
            Self::Exp { coef: [a, b], ux } => vec![(a - b * ux).exp(), *b],
            Self::Quad {
                abc: [a, b, c],
                ux,
                uy,
            } => vec![c - b * ux + a * ux * ux + uy, b - 2.0 * a * ux, *a],
            // vega-statistics' `uncenter`: the polynomial in `x - ux`
            // expanded in `x`.
            Self::Poly { coef, ux, uy } => {
                let mut out = vec![0.0; coef.len()];
                for (i, &v) in coef.iter().enumerate().rev() {
                    let mut c = 1.0;
                    out[i] += v;
                    for j in 1..=i {
                        c *= (i + 1 - j) as f64 / j as f64;
                        out[i - j] += v * number::pow(-ux, j as f64) * c;
                    }
                }
                out[0] += uy;
                out
            }
        }
    }

    /// The share of the variance in `y` the fit explains over the pairs
    /// where both values are numbers; 0 for a constant.
    fn r_squared(&self, x: &[f64], y: &[f64]) -> f64 {
        if let Self::Constant(_) = self {
            return 0.0;
        }
        let mut mean = 0.0;
        for (n, (_, y)) in Self::pairs(x, y).enumerate() {
            mean += (y - mean) / (n + 1) as f64;
        }
        let (mut sse, mut sst) = (0.0, 0.0);
        for (x, y) in Self::pairs(x, y) {
            let (e, t) = (y - self.predict(x), y - mean);
            sse += e * e;
            sst += t * t;
        }
        1.0 - sse / sst
    }

    /// Ordinary least squares `[intercept, slope]` from the means of x, y,
    /// x·y and x².
    fn ols(x: f64, y: f64, xy: f64, x2: f64) -> [f64; 2] {
        let delta = x2 - x * x;
        let slope = if delta.abs() < 1e-24 {
            0.0
        } else {
            (xy - x * y) / delta
        };
        [y - slope * x, slope]
    }

    /// The pairs where both values are numbers. Vega's `visitPoints`.
    fn pairs<'a>(x: &'a [f64], y: &'a [f64]) -> impl Iterator<Item = (f64, f64)> + 'a {
        x.iter()
            .zip(y)
            .map(|(x, y)| (*x, *y))
            .filter(|(x, y)| !x.is_nan() && !y.is_nan())
    }

    /// The pairs centered on their means, and the means. Vega's `points`.
    fn centered(x: &[f64], y: &[f64]) -> (Vec<f64>, Vec<f64>, f64, f64) {
        let (mut xs, mut ys) = (Vec::new(), Vec::new());
        let (mut ux, mut uy) = (0.0, 0.0);
        for (i, (x, y)) in Self::pairs(x, y).enumerate() {
            let i = (i + 1) as f64;
            ux += (x - ux) / i;
            uy += (y - uy) / i;
            xs.push(x);
            ys.push(y);
        }
        xs.iter_mut().for_each(|x| *x -= ux);
        ys.iter_mut().for_each(|y| *y -= uy);
        (xs, ys, ux, uy)
    }

    /// OLS over the pairs mapped by `map_x` and `map_y`, its means taken
    /// running.
    fn means(
        x: &[f64],
        y: &[f64],
        map_x: impl Fn(f64) -> f64,
        map_y: impl Fn(f64) -> f64,
    ) -> [f64; 2] {
        let (mut ux, mut uy, mut uxy, mut ux2) = (0.0, 0.0, 0.0, 0.0);
        for (n, (x, y)) in Self::pairs(x, y).enumerate() {
            let n = (n + 1) as f64;
            let (x, y) = (map_x(x), map_y(y));
            ux += (x - ux) / n;
            uy += (y - uy) / n;
            uxy += (x * y - uxy) / n;
            ux2 += (x * x - ux2) / n;
        }
        Self::ols(ux, uy, uxy, ux2)
    }

    fn constant(x: &[f64], y: &[f64]) -> Self {
        let mut mean = 0.0;
        for (n, (_, y)) in Self::pairs(x, y).enumerate() {
            mean += (y - mean) / (n + 1) as f64;
        }
        Self::Constant(mean)
    }

    fn exp(x: &[f64], y: &[f64]) -> Self {
        let (xv, _, ux, uy) = Self::centered(x, y);
        let (mut yl, mut xy, mut xyl, mut x2y) = (0.0, 0.0, 0.0, 0.0);
        for (i, (_, dy)) in Self::pairs(x, y).enumerate() {
            let dx = xv[i];
            let n = (i + 1) as f64;
            let ly = dy.ln();
            let product = dx * dy;
            yl += (dy * ly - yl) / n;
            xy += (product - xy) / n;
            xyl += (product * ly - xyl) / n;
            x2y += (dx * product - x2y) / n;
        }
        let coef = Self::ols(xy / uy, yl / uy, xyl / uy, x2y / uy);
        Self::Exp { coef, ux }
    }

    fn quad(x: &[f64], y: &[f64]) -> Self {
        let (xv, yv, ux, uy) = Self::centered(x, y);
        let (mut x2, mut x3, mut x4, mut xy, mut x2y) = (0.0, 0.0, 0.0, 0.0, 0.0);
        for (i, (dx, dy)) in xv.iter().zip(&yv).enumerate() {
            let n = (i + 1) as f64;
            let sq = dx * dx;
            x2 += (sq - x2) / n;
            x3 += (sq * dx - x3) / n;
            x4 += (sq * sq - x4) / n;
            xy += (dx * dy - xy) / n;
            x2y += (sq * dy - x2y) / n;
        }
        let x2x2 = x4 - x2 * x2;
        let d = x2 * x2x2 - x3 * x3;
        let a = (x2y * x2 - xy * x3) / d;
        let b = (xy * x2x2 - x2y * x3) / d;
        let c = -a * x2;
        Self::Quad {
            abc: [a, b, c],
            ux,
            uy,
        }
    }

    /// Normal equations solved by Gaussian elimination, as Vega's
    /// `regressionPoly` for orders above 2.
    fn poly(x: &[f64], y: &[f64], order: usize) -> Self {
        let (xv, yv, ux, uy) = Self::centered(x, y);
        let k = order + 1;
        // Columns: `matrix[column][row]`, the right-hand side last.
        let mut matrix: Vec<Vec<f64>> = Vec::with_capacity(k + 1);
        let mut rhs = Vec::with_capacity(k);
        for i in 0..k {
            rhs.push(
                xv.iter()
                    .zip(&yv)
                    .map(|(x, y)| x.powf(i as f64) * y)
                    .sum::<f64>(),
            );
            matrix.push(
                (0..k)
                    .map(|j| xv.iter().map(|x| x.powf((i + j) as f64)).sum())
                    .collect(),
            );
        }
        matrix.push(rhs);

        let n = k;
        for i in 0..n {
            let mut r = i;
            for j in i + 1..n {
                if matrix[i][j].abs() > matrix[i][r].abs() {
                    r = j;
                }
            }
            for column in &mut matrix[i..=n] {
                column.swap(i, r);
            }
            for j in i + 1..n {
                for col in (i..=n).rev() {
                    let pivot = matrix[col][i] * matrix[i][j] / matrix[i][i];
                    matrix[col][j] -= pivot;
                }
            }
        }
        let mut coef = vec![0.0; n];
        for j in (0..n).rev() {
            let t: f64 = (j + 1..n).map(|col| matrix[col][j] * coef[col]).sum();
            coef[j] = (matrix[n][j] - t) / matrix[j][j];
        }
        Self::Poly { coef, ux, uy }
    }
}
