//! vega-statistics' regressions: least-squares fits and loess.

use d3::{array, format::number, math};

/// Robustness iterations after the first fit.
const ITERATIONS: usize = 2;
const EPSILON: f64 = 1e-12;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Method {
    /// `y = a`
    Constant,
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

/// A fitted curve, as Vega's `regression*` functions compute it.
pub enum Fit {
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
    /// The fit of `y` on `x` over the pairs where both are numbers.
    pub fn new(method: Method, x: &[f64], y: &[f64]) -> Self {
        match method {
            Method::Linear | Method::Poly(1) => Self::Linear(Self::means(x, y, |x| x, |y| y)),
            Method::Log => Self::Log(Self::means(x, y, math::log, |y| y)),
            Method::Exp => Self::exp(x, y),
            Method::Pow => {
                let [a, b] = Self::means(x, y, math::log, math::log);
                Self::Pow([math::exp(a), b])
            }
            Method::Quad | Method::Poly(2) => Self::quad(x, y),
            Method::Constant | Method::Poly(0) => Self::constant(x, y),
            Method::Poly(order) => Self::poly(x, y, order),
        }
    }

    pub fn predict(&self, x: f64) -> f64 {
        match self {
            Self::Constant(mean) => *mean,
            Self::Linear([a, b]) => a + b * x,
            Self::Log([a, b]) => a + b * math::log(x),
            Self::Exp { coef: [a, b], ux } => math::exp(a + b * (x - ux)),
            Self::Pow([a, b]) => a * number::pow(x, *b),
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
                    y += c * number::pow(x, i as f64);
                }
                y
            }
        }
    }

    /// Vega's `coef`: the coefficients of `x`'s powers, lowest first, but an
    /// exponential's `[a, b]` of `a·e^(b·x)` and a power's of `a·x^b`.
    pub fn coef(&self) -> Vec<f64> {
        match self {
            Self::Constant(mean) => vec![*mean],
            Self::Linear(coef) | Self::Log(coef) | Self::Pow(coef) => coef.to_vec(),
            Self::Exp { coef: [a, b], ux } => vec![math::exp(a - b * ux), *b],
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
    pub fn r_squared(&self, x: &[f64], y: &[f64]) -> f64 {
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
            let ly = math::log(dy);
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
                    .map(|(x, y)| number::pow(*x, i as f64) * y)
                    .sum::<f64>(),
            );
            matrix.push(
                (0..k)
                    .map(|j| xv.iter().map(|x| number::pow(*x, (i + j) as f64)).sum())
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

/// `regressionLoess` over `pairs` ascending by x: one point per distinct x,
/// its fits averaged.
pub fn loess(pairs: &[[f64; 2]], bandwidth: f64) -> Vec<[f64; 2]> {
    let n = pairs.len();
    let (mut ux, mut uy) = (0.0, 0.0);
    for (i, [x, y]) in pairs.iter().enumerate() {
        let i = (i + 1) as f64;
        ux += (x - ux) / i;
        uy += (y - uy) / i;
    }
    let xv: Vec<f64> = pairs.iter().map(|[x, _]| x - ux).collect();
    let yv: Vec<f64> = pairs.iter().map(|[_, y]| y - uy).collect();
    // Out of range reads as `NaN`, as an array past its end does in Vega.
    let at = |values: &[f64], i: usize| values.get(i).copied().unwrap_or(f64::NAN);
    let neighbours = 2.max((bandwidth * n as f64) as usize);
    let mut fit = vec![0.0; n];
    let mut residuals = vec![0.0; n];
    let mut robust = vec![1.0; n];

    for iteration in 0..=ITERATIONS {
        let [mut left, mut right] = [0, neighbours - 1];
        for i in 0..n {
            let dx = xv[i];
            let edge = if dx - at(&xv, left) > at(&xv, right) - dx {
                left
            } else {
                right
            };
            let span = at(&xv, edge) - dx;
            let span = if span == 0.0 || span.is_nan() {
                1.0
            } else {
                span
            };
            let denom = 1.0 / span.abs();
            let (mut w_sum, mut x_sum, mut y_sum, mut xy_sum, mut x2_sum) =
                (0.0, 0.0, 0.0, 0.0, 0.0);
            for k in left..=right {
                let (xk, yk) = (at(&xv, k), at(&yv, k));
                let t = (dx - xk).abs() * denom;
                let t = 1.0 - t * t * t;
                let w = t * t * t * at(&robust, k);
                let xkw = xk * w;
                w_sum += w;
                x_sum += xkw;
                y_sum += yk * w;
                xy_sum += yk * xkw;
                x2_sum += xk * xkw;
            }
            let (mx, my) = (x_sum / w_sum, y_sum / w_sum);
            let (mxy, mx2) = (xy_sum / w_sum, x2_sum / w_sum);
            let delta = mx2 - mx * mx;
            let slope = if delta.abs() < 1e-24 {
                0.0
            } else {
                (mxy - mx * my) / delta
            };
            fit[i] = my - slope * mx + slope * dx;
            residuals[i] = (yv[i] - fit[i]).abs();

            // Slide the window right while its next point is no farther than
            // its first.
            let next = i + 1;
            let mut far = right + 1;
            if far < n && next < n {
                let value = xv[next];
                while next > left && far < n && xv[far] - value <= value - xv[left] {
                    left += 1;
                    right = far;
                    far += 1;
                }
            }
        }
        if iteration == ITERATIONS {
            break;
        }
        let mut sorted: Vec<f64> = residuals.iter().copied().filter(|r| !r.is_nan()).collect();
        sorted.sort_by(f64::total_cmp);
        let median = array::quantile_sorted(&sorted, 0.5);
        if median.abs() < EPSILON {
            break;
        }
        for (weight, residual) in robust.iter_mut().zip(&residuals) {
            let arg = residual / (6.0 * median);
            *weight = if arg >= 1.0 {
                EPSILON
            } else {
                let w = 1.0 - arg * arg;
                w * w
            };
        }
    }

    let mut out: Vec<[f64; 2]> = Vec::new();
    let mut repeats = 0.0;
    for (x, fit) in xv.iter().zip(&fit) {
        let x = x + ux;
        match out.last_mut() {
            Some(last) if last[0] == x => {
                repeats += 1.0;
                last[1] += (fit - last[1]) / repeats;
            }
            last => {
                if let Some(last) = last {
                    last[1] += uy;
                }
                repeats = 0.0;
                out.push([x, *fit]);
            }
        }
    }
    if let Some(last) = out.last_mut() {
        last[1] += uy;
    }
    out
}
