//! d3-array's `ticks`, `tickIncrement`, `tickStep`, `nice`, `bisectRight`
//! and `quantileSorted`, and JavaScript's `Array.prototype.sort`.

use crate::format::number;
use std::cmp::Ordering;

const E10: f64 = 7.0710678118654755;
const E5: f64 = 3.1622776601683795;
const E2: f64 = std::f64::consts::SQRT_2;

/// The first and last tick's index and the increment: a step of
/// `10^power * factor` as is, a step below one as the negative of its
/// inverse.
fn tick_spec(start: f64, stop: f64, count: f64) -> (f64, f64, f64) {
    let step = (stop - start) / count.max(0.0);
    let power = step.log10().floor();
    let error = step / number::pow(10.0, power);
    let factor = if error >= E10 {
        10.0
    } else if error >= E5 {
        5.0
    } else if error >= E2 {
        2.0
    } else {
        1.0
    };
    let (mut i1, mut i2, inc);
    if power < 0.0 {
        let mut inverse = number::pow(10.0, -power) / factor;
        i1 = number::round(start * inverse);
        i2 = number::round(stop * inverse);
        if i1 / inverse < start {
            i1 += 1.0;
        }
        if i2 / inverse > stop {
            i2 -= 1.0;
        }
        inverse = -inverse;
        inc = inverse;
    } else {
        inc = number::pow(10.0, power) * factor;
        i1 = number::round(start / inc);
        i2 = number::round(stop / inc);
        if i1 * inc < start {
            i1 += 1.0;
        }
        if i2 * inc > stop {
            i2 -= 1.0;
        }
    }
    if i2 < i1 && (0.5..2.0).contains(&count) {
        return tick_spec(start, stop, count * 2.0);
    }
    (i1, i2, inc)
}

/// `d3.ticks(start, stop, count)`: about `count` round values from `start`
/// to `stop`, in their order.
pub fn ticks(start: f64, stop: f64, count: f64) -> Vec<f64> {
    if count.is_nan() || count <= 0.0 {
        return Vec::new();
    }
    if start == stop {
        return vec![start];
    }
    let reverse = stop < start;
    let (i1, i2, inc) = match reverse {
        true => tick_spec(stop, start, count),
        false => tick_spec(start, stop, count),
    };
    if i1.is_nan() || i2.is_nan() || i2 < i1 {
        return Vec::new();
    }
    let n = (i2 - i1 + 1.0) as usize;
    (0..n)
        .map(|i| {
            let index = match reverse {
                true => i2 - i as f64,
                false => i1 + i as f64,
            };
            match inc < 0.0 {
                true => index / -inc,
                false => index * inc,
            }
        })
        .collect()
}

/// `d3.tickIncrement(start, stop, count)`: the tick step, a step below one
/// as the negative of its inverse. `start` is at most `stop`.
pub fn tick_increment(start: f64, stop: f64, count: f64) -> f64 {
    tick_spec(start, stop, count).2
}

/// `d3.tickStep(start, stop, count)`: the tick step, negative for a
/// `stop` before `start`.
pub fn tick_step(start: f64, stop: f64, count: f64) -> f64 {
    let reverse = stop < start;
    let inc = match reverse {
        true => tick_increment(stop, start, count),
        false => tick_increment(start, stop, count),
    };
    let step = match inc < 0.0 {
        true => 1.0 / -inc,
        false => inc,
    };
    match reverse {
        true => -step,
        false => step,
    }
}

/// `d3.nice(start, stop, count)`: `start..stop` widened to the ticks either
/// side.
pub fn nice(mut start: f64, mut stop: f64, count: f64) -> (f64, f64) {
    let mut previous = None;
    loop {
        let step = tick_increment(start, stop, count);
        if previous == Some(step) || step == 0.0 || !step.is_finite() {
            return (start, stop);
        }
        if step > 0.0 {
            start = (start / step).floor() * step;
            stop = (stop / step).ceil() * step;
        } else {
            start = (start * step).ceil() / step;
            stop = (stop * step).floor() / step;
        }
        previous = Some(step);
    }
}

/// `d3.bisectRight(values, x, lo, hi)`: where `x` goes in ascending
/// `values[lo..hi]`, after any equal to it.
pub fn bisect_right(values: &[f64], x: f64, lo: usize, hi: usize) -> usize {
    if x.is_nan() {
        return hi;
    }
    let (mut lo, mut hi) = (lo, hi);
    while lo < hi {
        let mid = (lo + hi) / 2;
        // d3 compares with `ascending`, which puts NaN nowhere.
        if values[mid] <= x {
            lo = mid + 1;
        } else {
            hi = mid;
        }
    }
    lo
}

/// `d3.quantileSorted(values, p)`: the `p`-quantile of ascending `values`
/// by R-7. `NaN` for none.
pub fn quantile_sorted(values: &[f64], p: f64) -> f64 {
    let n = values.len();
    if n == 0 || p.is_nan() {
        return f64::NAN;
    }
    if p <= 0.0 || n < 2 {
        return values[0];
    }
    if p >= 1.0 {
        return values[n - 1];
    }
    let i = (n - 1) as f64 * p;
    let i0 = i.floor() as usize;
    let (value0, value1) = (values[i0], values[i0 + 1]);
    value0 + (value1 - value0) * (i - i0 as f64)
}

/// `Array.prototype.sort` with a comparator, as V8's TimSort runs it, so
/// that a comparator that is not a total order leaves the items as V8
/// leaves them. Never panics.
pub fn sort_by<T: Clone>(items: &mut [T], compare: impl FnMut(&T, &T) -> Ordering) {
    TimSort {
        items,
        compare,
        runs: Vec::new(),
        min_gallop: MIN_GALLOP,
    }
    .sort();
}

/// Wins in a row before a merge gallops.
const MIN_GALLOP: usize = 7;

struct TimSort<'a, T, F> {
    items: &'a mut [T],
    compare: F,
    /// Pending runs: their bases and lengths.
    runs: Vec<(usize, usize)>,
    min_gallop: usize,
}

impl<T: Clone, F: FnMut(&T, &T) -> Ordering> TimSort<'_, T, F> {
    fn less(&mut self, a: &T, b: &T) -> bool {
        (self.compare)(a, b) == Ordering::Less
    }

    fn sort(mut self) {
        let length = self.items.len();
        if length < 2 {
            return;
        }
        let min_run = min_run_length(length);
        let (mut low, mut remaining) = (0, length);
        while remaining != 0 {
            let mut run = self.count_and_make_run(low, low + remaining);
            if run < min_run {
                let forced = min_run.min(remaining);
                self.binary_insertion_sort(low, low + run, low + forced);
                run = forced;
            }
            self.runs.push((low, run));
            self.merge_collapse();
            low += run;
            remaining -= run;
        }
        while self.runs.len() > 1 {
            let mut n = self.runs.len() - 2;
            if n > 0 && self.runs[n - 1].1 < self.runs[n + 1].1 {
                n -= 1;
            }
            self.merge_at(n);
        }
    }

    fn binary_insertion_sort(&mut self, low: usize, start: usize, high: usize) {
        let mut start = if low == start { start + 1 } else { start };
        while start < high {
            let (mut left, mut right) = (low, start);
            let pivot = self.items[right].clone();
            while left < right {
                let mid = left + ((right - left) >> 1);
                let item = self.items[mid].clone();
                if self.less(&pivot, &item) {
                    right = mid;
                } else {
                    left = mid + 1;
                }
            }
            self.items[left..=start].rotate_right(1);
            self.items[left] = pivot;
            start += 1;
        }
    }

    fn count_and_make_run(&mut self, low: usize, high: usize) -> usize {
        let next = low + 1;
        if next == high {
            return 1;
        }
        let mut length = 2;
        let (first, second) = (self.items[next].clone(), self.items[next - 1].clone());
        let descending = self.less(&first, &second);
        let mut previous = first;
        for i in next + 1..high {
            let current = self.items[i].clone();
            let less = self.less(&current, &previous);
            if descending != less {
                break;
            }
            previous = current;
            length += 1;
        }
        if descending {
            self.items[low..low + length].reverse();
        }
        length
    }

    fn merge_collapse(&mut self) {
        let established =
            |runs: &[(usize, usize)], n: usize| n < 2 || runs[n - 2].1 > runs[n - 1].1 + runs[n].1;
        while self.runs.len() > 1 {
            let mut n = self.runs.len() - 2;
            if !established(&self.runs, n + 1) || !established(&self.runs, n) {
                if n > 0 && self.runs[n - 1].1 < self.runs[n + 1].1 {
                    n -= 1;
                }
                self.merge_at(n);
            } else if self.runs[n].1 <= self.runs[n + 1].1 {
                self.merge_at(n);
            } else {
                break;
            }
        }
    }

    fn merge_at(&mut self, i: usize) {
        let (mut base_a, mut length_a) = self.runs[i];
        let (base_b, mut length_b) = self.runs[i + 1];
        self.runs[i].1 = length_a + length_b;
        self.runs.remove(i + 1);
        let key = self.items[base_b].clone();
        let k = self.gallop_right(Side::Items, &key, base_a, length_a, 0, &[]);
        base_a += k;
        length_a -= k;
        if length_a == 0 {
            return;
        }
        let key = self.items[base_a + length_a - 1].clone();
        length_b = self.gallop_left(Side::Items, &key, base_b, length_b, length_b - 1, &[]);
        if length_b == 0 {
            return;
        }
        if length_a <= length_b {
            self.merge_low(base_a, length_a, base_b, length_b);
        } else {
            self.merge_high(base_a, length_a, base_b, length_b);
        }
    }

    fn at<'s>(&'s self, side: Side, temp: &'s [T], i: usize) -> &'s T {
        match side {
            Side::Items => &self.items[i],
            Side::Temp => &temp[i],
        }
    }

    /// Where `key` goes in the run at `base`, before any equal item.
    fn gallop_left(
        &mut self,
        side: Side,
        key: &T,
        base: usize,
        length: usize,
        hint: usize,
        temp: &[T],
    ) -> usize {
        let (mut last, mut offset) = (0isize, 1isize);
        let (length_i, hint_i) = (length as isize, hint as isize);
        let item = self.at(side, temp, base + hint).clone();
        if self.less(&item, key) {
            let max = length_i - hint_i;
            while offset < max {
                let item = self.at(side, temp, base + hint + offset as usize).clone();
                if !self.less(&item, key) {
                    break;
                }
                last = offset;
                offset = (offset << 1) + 1;
                if offset <= 0 {
                    offset = max;
                }
            }
            offset = offset.min(max);
            last += hint_i;
            offset += hint_i;
        } else {
            let max = hint_i + 1;
            while offset < max {
                let item = self.at(side, temp, base + hint - offset as usize).clone();
                if self.less(&item, key) {
                    break;
                }
                last = offset;
                offset = (offset << 1) + 1;
                if offset <= 0 {
                    offset = max;
                }
            }
            offset = offset.min(max);
            let tmp = last;
            last = hint_i - offset;
            offset = hint_i - tmp;
        }
        last += 1;
        while last < offset {
            let m = last + ((offset - last) >> 1);
            let item = self.at(side, temp, base + m as usize).clone();
            if self.less(&item, key) {
                last = m + 1;
            } else {
                offset = m;
            }
        }
        offset as usize
    }

    /// Where `key` goes in the run at `base`, after any equal item.
    fn gallop_right(
        &mut self,
        side: Side,
        key: &T,
        base: usize,
        length: usize,
        hint: usize,
        temp: &[T],
    ) -> usize {
        let (mut last, mut offset) = (0isize, 1isize);
        let (length_i, hint_i) = (length as isize, hint as isize);
        let item = self.at(side, temp, base + hint).clone();
        if self.less(key, &item) {
            let max = hint_i + 1;
            while offset < max {
                let item = self.at(side, temp, base + hint - offset as usize).clone();
                if !self.less(key, &item) {
                    break;
                }
                last = offset;
                offset = (offset << 1) + 1;
                if offset <= 0 {
                    offset = max;
                }
            }
            offset = offset.min(max);
            let tmp = last;
            last = hint_i - offset;
            offset = hint_i - tmp;
        } else {
            let max = length_i - hint_i;
            while offset < max {
                let item = self.at(side, temp, base + hint + offset as usize).clone();
                if self.less(key, &item) {
                    break;
                }
                last = offset;
                offset = (offset << 1) + 1;
                if offset <= 0 {
                    offset = max;
                }
            }
            offset = offset.min(max);
            last += hint_i;
            offset += hint_i;
        }
        last += 1;
        while last < offset {
            let m = last + ((offset - last) >> 1);
            let item = self.at(side, temp, base + m as usize).clone();
            if self.less(key, &item) {
                offset = m;
            } else {
                last = m + 1;
            }
        }
        offset as usize
    }

    fn merge_low(
        &mut self,
        base_a: usize,
        mut length_a: usize,
        base_b: usize,
        mut length_b: usize,
    ) {
        let temp: Vec<T> = self.items[base_a..base_a + length_a].to_vec();
        let (mut dest, mut cursor_temp, mut cursor_b) = (base_a, 0, base_b);
        self.items[dest] = self.items[cursor_b].clone();
        dest += 1;
        cursor_b += 1;
        length_b -= 1;
        // Whether A is down to its last item, which goes after what is left
        // of B.
        let copy_b = 'merge: {
            if length_b == 0 {
                break 'merge false;
            }
            if length_a == 1 {
                break 'merge true;
            }
            let mut min_gallop = self.min_gallop;
            loop {
                let (mut wins_a, mut wins_b) = (0, 0);
                loop {
                    let (b, a) = (self.items[cursor_b].clone(), temp[cursor_temp].clone());
                    if self.less(&b, &a) {
                        self.items[dest] = b;
                        dest += 1;
                        cursor_b += 1;
                        wins_b += 1;
                        length_b -= 1;
                        wins_a = 0;
                        if length_b == 0 {
                            break 'merge false;
                        }
                        if wins_b >= min_gallop {
                            break;
                        }
                    } else {
                        self.items[dest] = a;
                        dest += 1;
                        cursor_temp += 1;
                        wins_a += 1;
                        length_a -= 1;
                        wins_b = 0;
                        if length_a == 1 {
                            break 'merge true;
                        }
                        if wins_a >= min_gallop {
                            break;
                        }
                    }
                }
                min_gallop += 1;
                let mut first = true;
                while wins_a >= MIN_GALLOP || wins_b >= MIN_GALLOP || first {
                    first = false;
                    min_gallop = min_gallop.saturating_sub(1).max(1);
                    self.min_gallop = min_gallop;
                    let key = self.items[cursor_b].clone();
                    wins_a = self.gallop_right(Side::Temp, &key, cursor_temp, length_a, 0, &temp);
                    if wins_a > 0 {
                        self.items[dest..dest + wins_a]
                            .clone_from_slice(&temp[cursor_temp..cursor_temp + wins_a]);
                        dest += wins_a;
                        cursor_temp += wins_a;
                        length_a -= wins_a;
                        if length_a == 1 {
                            break 'merge true;
                        }
                        if length_a == 0 {
                            break 'merge false;
                        }
                    }
                    self.items[dest] = self.items[cursor_b].clone();
                    dest += 1;
                    cursor_b += 1;
                    length_b -= 1;
                    if length_b == 0 {
                        break 'merge false;
                    }
                    let key = temp[cursor_temp].clone();
                    wins_b = self.gallop_left(Side::Items, &key, cursor_b, length_b, 0, &[]);
                    if wins_b > 0 {
                        for k in 0..wins_b {
                            self.items[dest + k] = self.items[cursor_b + k].clone();
                        }
                        dest += wins_b;
                        cursor_b += wins_b;
                        length_b -= wins_b;
                        if length_b == 0 {
                            break 'merge false;
                        }
                    }
                    self.items[dest] = temp[cursor_temp].clone();
                    dest += 1;
                    cursor_temp += 1;
                    length_a -= 1;
                    if length_a == 1 {
                        break 'merge true;
                    }
                }
                min_gallop += 1;
                self.min_gallop = min_gallop;
            }
        };
        if copy_b {
            for k in 0..length_b {
                self.items[dest + k] = self.items[cursor_b + k].clone();
            }
            self.items[dest + length_b] = temp[cursor_temp].clone();
        } else if length_a > 0 {
            self.items[dest..dest + length_a]
                .clone_from_slice(&temp[cursor_temp..cursor_temp + length_a]);
        }
    }

    fn merge_high(
        &mut self,
        base_a: usize,
        mut length_a: usize,
        base_b: usize,
        mut length_b: usize,
    ) {
        let temp: Vec<T> = self.items[base_b..base_b + length_b].to_vec();
        // Cursors as signed, since they run down past their runs' starts.
        let mut dest = (base_b + length_b - 1) as isize;
        let mut cursor_temp = length_b as isize - 1;
        let mut cursor_a = (base_a + length_a - 1) as isize;
        self.items[dest as usize] = self.items[cursor_a as usize].clone();
        dest -= 1;
        cursor_a -= 1;
        length_a -= 1;
        // Whether B is down to its first item, which goes before what is
        // left of A.
        let copy_a = 'merge: {
            if length_a == 0 {
                break 'merge false;
            }
            if length_b == 1 {
                break 'merge true;
            }
            let mut min_gallop = self.min_gallop;
            loop {
                let (mut wins_a, mut wins_b) = (0, 0);
                loop {
                    let (b, a) = (
                        temp[cursor_temp as usize].clone(),
                        self.items[cursor_a as usize].clone(),
                    );
                    if self.less(&b, &a) {
                        self.items[dest as usize] = a;
                        dest -= 1;
                        cursor_a -= 1;
                        wins_a += 1;
                        length_a -= 1;
                        wins_b = 0;
                        if length_a == 0 {
                            break 'merge false;
                        }
                        if wins_a >= min_gallop {
                            break;
                        }
                    } else {
                        self.items[dest as usize] = b;
                        dest -= 1;
                        cursor_temp -= 1;
                        wins_b += 1;
                        length_b -= 1;
                        wins_a = 0;
                        if length_b == 1 {
                            break 'merge true;
                        }
                        if wins_b >= min_gallop {
                            break;
                        }
                    }
                }
                min_gallop += 1;
                let mut first = true;
                while wins_a >= MIN_GALLOP || wins_b >= MIN_GALLOP || first {
                    first = false;
                    min_gallop = min_gallop.saturating_sub(1).max(1);
                    self.min_gallop = min_gallop;
                    let key = temp[cursor_temp as usize].clone();
                    let k =
                        self.gallop_right(Side::Items, &key, base_a, length_a, length_a - 1, &[]);
                    wins_a = length_a - k;
                    if wins_a > 0 {
                        dest -= wins_a as isize;
                        cursor_a -= wins_a as isize;
                        for j in (0..wins_a).rev() {
                            self.items[(dest + 1) as usize + j] =
                                self.items[(cursor_a + 1) as usize + j].clone();
                        }
                        length_a -= wins_a;
                        if length_a == 0 {
                            break 'merge false;
                        }
                    }
                    self.items[dest as usize] = temp[cursor_temp as usize].clone();
                    dest -= 1;
                    cursor_temp -= 1;
                    length_b -= 1;
                    if length_b == 1 {
                        break 'merge true;
                    }
                    let key = self.items[cursor_a as usize].clone();
                    let k = self.gallop_left(Side::Temp, &key, 0, length_b, length_b - 1, &temp);
                    wins_b = length_b - k;
                    if wins_b > 0 {
                        dest -= wins_b as isize;
                        cursor_temp -= wins_b as isize;
                        self.items[(dest + 1) as usize..(dest + 1) as usize + wins_b]
                            .clone_from_slice(
                                &temp[(cursor_temp + 1) as usize
                                    ..(cursor_temp + 1) as usize + wins_b],
                            );
                        length_b -= wins_b;
                        if length_b == 1 {
                            break 'merge true;
                        }
                        if length_b == 0 {
                            break 'merge false;
                        }
                    }
                    self.items[dest as usize] = self.items[cursor_a as usize].clone();
                    dest -= 1;
                    cursor_a -= 1;
                    length_a -= 1;
                    if length_a == 0 {
                        break 'merge false;
                    }
                }
                min_gallop += 1;
                self.min_gallop = min_gallop;
            }
        };
        if copy_a {
            dest -= length_a as isize;
            cursor_a -= length_a as isize;
            for j in (0..length_a).rev() {
                self.items[(dest + 1) as usize + j] =
                    self.items[(cursor_a + 1) as usize + j].clone();
            }
            self.items[dest as usize] = temp[cursor_temp as usize].clone();
        } else if length_b > 0 {
            let start = (dest - (length_b as isize - 1)) as usize;
            self.items[start..start + length_b].clone_from_slice(&temp[..length_b]);
        }
    }
}

/// Which array a gallop reads.
#[derive(Clone, Copy)]
enum Side {
    Items,
    Temp,
}

/// TimSort's least run: `n` itself below 64, else between 32 and 64.
fn min_run_length(mut n: usize) -> usize {
    let mut r = 0;
    while n >= 64 {
        r |= n & 1;
        n >>= 1;
    }
    n + r
}
