//! vega-statistics' distributions: their densities and cumulative
//! probabilities.

use crate::{statistics, value::Value};
use d3::{format::number, math};
use std::f64::consts::PI;

/// A probability distribution.
#[derive(Clone, Debug, PartialEq)]
pub enum Distribution {
    Normal {
        mean: f64,
        stdev: f64,
    },
    Lognormal {
        mean: f64,
        stdev: f64,
    },
    Uniform {
        min: f64,
        max: f64,
    },
    /// A Gaussian kernel density over `support`.
    Kde {
        support: Vec<f64>,
        bandwidth: f64,
    },
    /// Each distribution weighted by its share of `weights`.
    Mixture {
        distributions: Vec<Distribution>,
        weights: Vec<f64>,
    },
}

impl Distribution {
    /// `randomKDE(values, bandwidth)`: a bandwidth of 0 or `NaN` is Scott's
    /// estimate. Each value reads as `+value`, null as 0.
    pub fn kde(values: &[Value], bandwidth: f64) -> Self {
        let bandwidth = match bandwidth != 0.0 && !bandwidth.is_nan() {
            true => bandwidth,
            false => scott(values),
        };
        Self::Kde {
            support: values.iter().map(Value::to_number).collect(),
            bandwidth,
        }
    }

    /// `randomMixture(distributions, weights)`: a missing weight is 1, and
    /// the weights are normalized to sum to 1.
    pub fn mixture(distributions: Vec<Distribution>, weights: &[Option<f64>]) -> Self {
        let raw: Vec<f64> = (0..distributions.len())
            .map(|i| weights.get(i).copied().flatten().unwrap_or(1.0))
            .collect();
        let sum: f64 = raw.iter().sum();
        Self::Mixture {
            distributions,
            weights: raw.iter().map(|w| w / sum).collect(),
        }
    }

    pub fn pdf(&self, x: f64) -> f64 {
        match self {
            Self::Normal { mean, stdev } => density_normal(x, *mean, *stdev),
            Self::Lognormal { mean, stdev } => density_log_normal(x, *mean, *stdev),
            Self::Uniform { min, max } => density_uniform(x, *min, *max),
            Self::Kde { support, bandwidth } => {
                let mut y = 0.0;
                for s in support {
                    y += density_normal((x - s) / bandwidth, 0.0, 1.0);
                }
                y / bandwidth / support.len() as f64
            }
            Self::Mixture {
                distributions,
                weights,
            } => {
                let mut p = 0.0;
                for (d, w) in distributions.iter().zip(weights) {
                    p += w * d.pdf(x);
                }
                p
            }
        }
    }

    pub fn cdf(&self, x: f64) -> f64 {
        match self {
            Self::Normal { mean, stdev } => cumulative_normal(x, *mean, *stdev),
            Self::Lognormal { mean, stdev } => cumulative_log_normal(x, *mean, *stdev),
            Self::Uniform { min, max } => cumulative_uniform(x, *min, *max),
            Self::Kde { support, bandwidth } => {
                let mut y = 0.0;
                for s in support {
                    y += cumulative_normal((x - s) / bandwidth, 0.0, 1.0);
                }
                y / support.len() as f64
            }
            Self::Mixture {
                distributions,
                weights,
            } => {
                let mut p = 0.0;
                for (d, w) in distributions.iter().zip(weights) {
                    p += w * d.cdf(x);
                }
                p
            }
        }
    }
}

/// `Math.sqrt(2 * Math.PI)`.
fn sqrt_2pi() -> f64 {
    (2.0 * PI).sqrt()
}

/// `densityNormal(value, mean, stdev)`.
pub fn density_normal(value: f64, mean: f64, stdev: f64) -> f64 {
    let z = (value - mean) / stdev;
    math::exp(-0.5 * z * z) / (stdev * sqrt_2pi())
}

/// `cumulativeNormal(value, mean, stdev)`: West's (2009) approximation.
pub fn cumulative_normal(value: f64, mean: f64, stdev: f64) -> f64 {
    let z = (value - mean) / stdev;
    let abs = z.abs();
    let cd = if abs > 37.0 {
        0.0
    } else {
        let exp = math::exp(-abs * abs / 2.0);
        if abs < 7.07106781186547 {
            let mut sum = 3.52624965998911e-02 * abs + 0.700383064443688;
            sum = sum * abs + 6.37396220353165;
            sum = sum * abs + 33.912866078383;
            sum = sum * abs + 112.079291497871;
            sum = sum * abs + 221.213596169931;
            sum = sum * abs + 220.206867912376;
            let cd = exp * sum;
            let mut sum = 8.83883476483184e-02 * abs + 1.75566716318264;
            sum = sum * abs + 16.064177579207;
            sum = sum * abs + 86.7807322029461;
            sum = sum * abs + 296.564248779674;
            sum = sum * abs + 637.333633378831;
            sum = sum * abs + 793.826512519948;
            sum = sum * abs + 440.413735824752;
            cd / sum
        } else {
            let mut sum = abs + 0.65;
            sum = abs + 4.0 / sum;
            sum = abs + 3.0 / sum;
            sum = abs + 2.0 / sum;
            sum = abs + 1.0 / sum;
            exp / sum / 2.506628274631
        }
    };
    if z > 0.0 { 1.0 - cd } else { cd }
}

/// `quantileNormal(p, mean, stdev)`: by the inverse error function, Giles'
/// (2010) approximation; `NaN` outside `[0, 1]`.
pub fn quantile_normal(p: f64, mean: f64, stdev: f64) -> f64 {
    if !(0.0..=1.0).contains(&p) {
        return f64::NAN;
    }
    mean + stdev * std::f64::consts::SQRT_2 * erfinv(2.0 * p - 1.0)
}

// Giles' coefficients, as vega-statistics writes them.
#[allow(clippy::excessive_precision)]
fn erfinv(x: f64) -> f64 {
    // The logarithm's argument as `(1 - x)(1 + x)`, not `1 - x²`, against
    // rounding near ±1.
    let mut w = -math::log((1.0 - x) * (1.0 + x));
    let horner = |w: f64, coefficients: &[f64]| {
        let mut p = coefficients[0];
        for c in &coefficients[1..] {
            p = c + p * w;
        }
        p
    };
    let p = if w < 6.25 {
        w -= 3.125;
        horner(
            w,
            &[
                -3.6444120640178196996e-21,
                -1.685059138182016589e-19,
                1.2858480715256400167e-18,
                1.115787767802518096e-17,
                -1.333171662854620906e-16,
                2.0972767875968561637e-17,
                6.6376381343583238325e-15,
                -4.0545662729752068639e-14,
                -8.1519341976054721522e-14,
                2.6335093153082322977e-12,
                -1.2975133253453532498e-11,
                -5.4154120542946279317e-11,
                1.051212273321532285e-09,
                -4.1126339803469836976e-09,
                -2.9070369957882005086e-08,
                4.2347877827932403518e-07,
                -1.3654692000834678645e-06,
                -1.3882523362786468719e-05,
                0.0001867342080340571352,
                -0.00074070253416626697512,
                -0.0060336708714301490533,
                0.24015818242558961693,
                1.6536545626831027356,
            ],
        )
    } else if w < 16.0 {
        w = w.sqrt() - 3.25;
        horner(
            w,
            &[
                2.2137376921775787049e-09,
                9.0756561938885390979e-08,
                -2.7517406297064545428e-07,
                1.8239629214389227755e-08,
                1.5027403968909827627e-06,
                -4.013867526981545969e-06,
                2.9234449089955446044e-06,
                1.2475304481671778723e-05,
                -4.7318229009055733981e-05,
                6.8284851459573175448e-05,
                2.4031110387097893999e-05,
                -0.0003550375203628474796,
                0.00095328937973738049703,
                -0.0016882755560235047313,
                0.0024914420961078508066,
                -0.0037512085075692412107,
                0.005370914553590063617,
                1.0052589676941592334,
                3.0838856104922207635,
            ],
        )
    } else if w.is_finite() {
        w = w.sqrt() - 5.0;
        horner(
            w,
            &[
                -2.7109920616438573243e-11,
                -2.5556418169965252055e-10,
                1.5076572693500548083e-09,
                -3.7894654401267369937e-09,
                7.6157012080783393804e-09,
                -1.4960026627149240478e-08,
                2.9147953450901080826e-08,
                -6.7711997758452339498e-08,
                2.2900482228026654717e-07,
                -9.9298272942317002539e-07,
                4.5260625972231537039e-06,
                -1.9681778105531670567e-05,
                7.5995277030017761139e-05,
                -0.00021503011930044477347,
                -0.00013871931833623122026,
                1.0103004648645343977,
                4.8499064014085844221,
            ],
        )
    } else {
        f64::INFINITY
    };
    p * x
}

/// `densityLogNormal(value, mean, stdev)`.
pub fn density_log_normal(value: f64, mean: f64, stdev: f64) -> f64 {
    if value <= 0.0 {
        return 0.0;
    }
    let z = (math::log(value) - mean) / stdev;
    math::exp(-0.5 * z * z) / (stdev * sqrt_2pi() * value)
}

/// `cumulativeLogNormal(value, mean, stdev)`.
pub fn cumulative_log_normal(value: f64, mean: f64, stdev: f64) -> f64 {
    cumulative_normal(math::log(value), mean, stdev)
}

/// `quantileLogNormal(p, mean, stdev)`.
pub fn quantile_log_normal(p: f64, mean: f64, stdev: f64) -> f64 {
    math::exp(quantile_normal(p, mean, stdev))
}

/// `densityUniform(value, min, max)`.
pub fn density_uniform(value: f64, min: f64, max: f64) -> f64 {
    match value >= min && value <= max {
        true => 1.0 / (max - min),
        false => 0.0,
    }
}

/// `cumulativeUniform(value, min, max)`.
pub fn cumulative_uniform(value: f64, min: f64, max: f64) -> f64 {
    if value < min {
        0.0
    } else if value > max {
        1.0
    } else {
        (value - min) / (max - min)
    }
}

/// `quantileUniform(p, min, max)`: `NaN` outside `[0, 1]`.
pub fn quantile_uniform(p: f64, min: f64, max: f64) -> f64 {
    match (0.0..=1.0).contains(&p) {
        true => min + p * (max - min),
        false => f64::NAN,
    }
}

/// `estimateBandwidth`: Scott's rule, `1.06 · min(σ, IQR / 1.34) · n^-0.2`,
/// falling back to σ, the first quartile's magnitude, then 1. `n` counts
/// every value; σ and the quartiles read only the numbers.
fn scott(values: &[Value]) -> f64 {
    let n = values.len() as f64;
    // d3-array's `deviation`: Welford's, over each value but null and
    // undefined as a number that is not `NaN`.
    let (mut count, mut mean, mut sum) = (0.0, 0.0, 0.0);
    for value in values.iter().filter(|v| !v.is_nullish()) {
        let value = value.to_number();
        if value.is_nan() {
            continue;
        }
        let delta = value - mean;
        count += 1.0;
        mean += delta / count;
        sum += delta * (value - mean);
    }
    let deviation = match count > 1.0 {
        true => (sum / (count - 1.0)).sqrt(),
        false => f64::NAN,
    };
    let [q1, _, q3] = statistics::quartiles(&statistics::numbers(values)).map(|q| q.to_number());
    let h = (q3 - q1) / 1.34;
    let truthy = |v: f64| v != 0.0 && !v.is_nan();
    // `Math.min` is `NaN` where either is.
    let min = match deviation.is_nan() || h.is_nan() {
        true => f64::NAN,
        false => deviation.min(h),
    };
    let v = [min, deviation, q1.abs()]
        .into_iter()
        .find(|v| truthy(*v))
        .unwrap_or(1.0);
    1.06 * v * number::pow(n, -0.2)
}
