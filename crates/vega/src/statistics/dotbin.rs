//! vega-statistics' `dotbin`: Wilkinson's dot density binning.

/// Each of `values`, ascending, at the center of its stack of width `step`;
/// with `smooth`, stacks within a quarter step of each other even out.
pub fn dotbin(values: &[f64], step: f64, smooth: bool) -> Vec<f64> {
    let n = values.len();
    let mut v = vec![0.0; n];
    if n == 0 {
        return v;
    }
    let (mut i, mut a) = (0, values[0]);
    let mut b = a;
    let mut w = a + step;
    for (j, &x) in values.iter().enumerate().skip(1) {
        if x >= w {
            b = (a + b) / 2.0;
            v[i..j].fill(b);
            i = j;
            w = x + step;
            a = x;
        }
        b = x;
    }
    b = (a + b) / 2.0;
    v[i..].fill(b);
    if smooth {
        smoothing(&mut v, step + step / 4.0);
    }
    v
}

/// Points swapped between adjacent stacks, to reduce their variance.
fn smoothing(v: &mut [f64], thresh: f64) {
    let n = v.len();
    let at = |v: &[f64], i: usize| v.get(i).copied().unwrap_or(f64::NAN);
    let (mut a, mut b) = (0, 1);
    while at(v, a) == at(v, b) {
        b += 1;
    }
    while b < n {
        let mut c = b + 1;
        while at(v, b) == at(v, c) {
            c += 1;
        }
        if v[b] - v[b - 1] < thresh {
            let mut d = (b as isize + ((a as isize + c as isize - 2 * b as isize) >> 1)) as usize;
            while d < b {
                v[d] = v[b];
                d += 1;
            }
            while d > b {
                v[d] = v[a];
                d -= 1;
            }
        }
        a = b;
        b = c;
    }
}
