//! vega-transforms' `Window`, with `WindowState` and `WindowOps`.

use crate::{
    Error,
    spec::Window,
    transform::{
        Accessor, Comparator, Key, Tuple,
        aggregate::{Aggregation, Op, measure_name},
        groups, or,
    },
    value::Value,
    view::View,
};
use std::cmp::Ordering;

/// A window op, by its Vega name.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum WindowOp {
    RowNumber,
    Rank,
    DenseRank,
    PercentRank,
    CumeDist,
    Ntile,
    Lag,
    Lead,
    FirstValue,
    LastValue,
    NthValue,
    PrevValue,
    NextValue,
}

impl WindowOp {
    fn parse(name: &str) -> Option<Self> {
        Some(match name {
            "row_number" => Self::RowNumber,
            "rank" => Self::Rank,
            "dense_rank" => Self::DenseRank,
            "percent_rank" => Self::PercentRank,
            "cume_dist" => Self::CumeDist,
            "ntile" => Self::Ntile,
            "lag" => Self::Lag,
            "lead" => Self::Lead,
            "first_value" => Self::FirstValue,
            "last_value" => Self::LastValue,
            "nth_value" => Self::NthValue,
            "prev_value" => Self::PrevValue,
            "next_value" => Self::NextValue,
            _ => return None,
        })
    }

    fn name(self) -> &'static str {
        match self {
            Self::RowNumber => "row_number",
            Self::Rank => "rank",
            Self::DenseRank => "dense_rank",
            Self::PercentRank => "percent_rank",
            Self::CumeDist => "cume_dist",
            Self::Ntile => "ntile",
            Self::Lag => "lag",
            Self::Lead => "lead",
            Self::FirstValue => "first_value",
            Self::LastValue => "last_value",
            Self::NthValue => "nth_value",
            Self::PrevValue => "prev_value",
            Self::NextValue => "next_value",
        }
    }
}

/// A window op of the transform, with its field, parameter and output.
struct Windowed {
    op: WindowOp,
    field: Option<Accessor>,
    param: f64,
    out: String,
}

/// Each window op's state over one partition.
#[derive(Clone, Default)]
struct Running {
    rank: f64,
    cume: f64,
    prev: Value,
    next: Value,
    next_index: isize,
}

/// The frame over a partition's sorted tuples: `[i0, i1)`, where it was,
/// and the tuple it is for.
#[derive(Default)]
struct Frame {
    i0: usize,
    i1: usize,
    p0: usize,
    p1: usize,
    index: usize,
}

pub fn window(view: &View, def: &Window, mut tuples: Vec<Tuple>) -> Result<Vec<Tuple>, Error> {
    let ops = or(view, &def.ops, Vec::new())?;
    let params = or(view, &def.params, Vec::new())?;
    let aggregate_params = or(view, &def.aggregate_params, Vec::new())?;
    let as_ = or(view, &def.as_, Vec::new())?;
    let mut windows = Vec::new();
    let (mut fields, mut measures, mut parameters, mut names) =
        (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    for (i, op) in ops.iter().enumerate() {
        let field = def.fields.get(i).cloned().flatten();
        match WindowOp::parse(op) {
            Some(op) => {
                let field = field.map(|f| Accessor::new(view, &f)).transpose()?;
                let out = measure_name(
                    op.name(),
                    field.as_ref().map(|f| f.name.as_str()),
                    as_.get(i).and_then(Option::as_deref),
                );
                let param = params.get(i).copied().flatten().unwrap_or(f64::NAN);
                match op {
                    WindowOp::Ntile if param.is_nan() || param <= 0.0 => {
                        return Err(Error::Spec("ntile num must be greater than zero.".into()));
                    }
                    WindowOp::NthValue if param.is_nan() || param <= 0.0 => {
                        return Err(Error::Spec(
                            "nth_value nth must be greater than zero.".into(),
                        ));
                    }
                    _ => {}
                }
                windows.push(Windowed {
                    op,
                    field,
                    param,
                    out,
                });
            }
            None => {
                fields.push(field.map(|f| Accessor::new(view, &f)).transpose()?);
                measures.push(Op::parse(op)?);
                parameters.push(aggregate_params.get(i).copied().flatten());
                names.push(as_.get(i).cloned().flatten());
            }
        }
    }
    let aggregation = (!measures.is_empty())
        .then(|| Aggregation::of(Vec::new(), None, fields, &measures, &parameters, &names))
        .transpose()?;
    let sort = def
        .sort
        .as_ref()
        .map(Comparator::new)
        .transpose()?
        .flatten();
    let range = sort.is_some() && !or(view, &def.ignore_peers, false)?;
    let frame = or(view, &def.frame, [None, Some(0.0)])?;
    let groupby = Accessor::many(view, &def.groupby)?;
    // The comparator without the `tupleid` tie break; `-1` without a sort.
    let compare = |a: &Tuple, b: &Tuple| -> Result<Ordering, Error> {
        match &sort {
            Some(sort) => sort.compare(view, &a.datum, &b.datum),
            None => Ok(Ordering::Less),
        }
    };
    for mut partition in groups(view, &groupby, Key::Cells, &tuples)? {
        if let Some(sort) = &sort {
            sort.sort_by(view, &mut partition, |&i| &tuples[i])?;
        }
        let data: Vec<Tuple> = partition.iter().map(|&i| tuples[i].clone()).collect();
        let n = data.len();
        let mut cell = aggregation.as_ref().map(Aggregation::cell);
        let mut running = vec![
            Running {
                rank: 1.0,
                prev: Value::Null,
                next: Value::Null,
                next_index: -1,
                ..Running::default()
            };
            windows.len()
        ];
        let mut w = Frame::default();
        for i in 0..n {
            w.p0 = w.i0;
            w.p1 = w.i1;
            let start = frame[0].map_or(0.0, |f| i as f64 + f);
            let end = frame[1].map_or(n as f64, |f| i as f64 + f + 1.0);
            w.i0 = start.clamp(0.0, n as f64) as usize;
            w.i1 = end.clamp(0.0, n as f64) as usize;
            w.index = i;
            if range {
                // Peers of the frame's ends join it.
                let (r0, r1) = (w.i0, w.i1 as isize - 1);
                if r0 > 0 && compare(&data[r0], &data[r0 - 1])? == Ordering::Equal {
                    w.i0 = bisect(&data, &data[r0], &compare, false)?;
                }
                if r1 >= 0
                    && (r1 as usize) < n - 1
                    && compare(&data[r1 as usize], &data[r1 as usize + 1])? == Ordering::Equal
                {
                    w.i1 = bisect(&data, &data[r1 as usize], &compare, true)?;
                }
            }
            let mut datum = tuples[partition[i]].datum.clone();
            if let Some(cell) = &mut cell {
                for t in &data[w.p0..w.i0.max(w.p0)] {
                    cell.rem(view, t)?;
                }
                for t in &data[w.p1..w.i1.max(w.p1)] {
                    cell.add(view, t)?;
                }
                datum = cell.set(view, datum)?;
            }
            for (window, state) in windows.iter().zip(&mut running) {
                let value = next(view, window, state, &w, &data, &compare)?;
                datum = datum.with_field(&window.out, value);
            }
            tuples[partition[i]].datum = datum;
        }
    }
    Ok(tuples)
}

/// d3-array's `bisector(compare).left` or `.right` of `x` in `data`.
fn bisect(
    data: &[Tuple],
    x: &Tuple,
    compare: &impl Fn(&Tuple, &Tuple) -> Result<Ordering, Error>,
    right: bool,
) -> Result<usize, Error> {
    let (mut lo, mut hi) = (0, data.len());
    while lo < hi {
        let mid = (lo + hi) / 2;
        let c = compare(&data[mid], x)?;
        let before = match right {
            true => c != Ordering::Greater,
            false => c == Ordering::Less,
        };
        if before {
            lo = mid + 1;
        } else {
            hi = mid;
        }
    }
    Ok(lo)
}

/// A window op's `next`: its value for the frame's tuple.
fn next(
    view: &View,
    window: &Windowed,
    state: &mut Running,
    w: &Frame,
    data: &[Tuple],
    compare: &impl Fn(&Tuple, &Tuple) -> Result<Ordering, Error>,
) -> Result<Value, Error> {
    let field = |t: &Tuple| match &window.field {
        Some(field) => field.get(view, &t.datum),
        None => Ok(Value::Undefined),
    };
    let n = data.len();
    let i = w.index;
    let differs = |a: usize, b: usize| -> Result<bool, Error> {
        Ok(compare(&data[a], &data[b])? != Ordering::Equal)
    };
    let rank = |state: &mut Running| -> Result<f64, Error> {
        if i > 0 && differs(i - 1, i)? {
            state.rank = i as f64 + 1.0;
        }
        Ok(state.rank)
    };
    let cume = |state: &mut Running| -> Result<f64, Error> {
        let mut j = i;
        if state.cume < j as f64 {
            while j + 1 < n && !differs(j, j + 1)? {
                j += 1;
            }
            state.cume = j as f64;
        }
        Ok((1.0 + state.cume) / n as f64)
    };
    let offset = if window.param != 0.0 && !window.param.is_nan() {
        window.param
    } else {
        1.0
    };
    Ok(match window.op {
        WindowOp::RowNumber => Value::Number(i as f64 + 1.0),
        WindowOp::Rank => Value::Number(rank(state)?),
        WindowOp::DenseRank => {
            if i > 0 && differs(i - 1, i)? {
                state.rank += 1.0;
            }
            Value::Number(state.rank)
        }
        WindowOp::PercentRank => Value::Number((rank(state)? - 1.0) / (n as f64 - 1.0)),
        WindowOp::CumeDist => Value::Number(cume(state)?),
        WindowOp::Ntile => Value::Number((window.param * cume(state)?).ceil()),
        WindowOp::Lag => {
            let j = i as f64 - offset;
            match j >= 0.0 {
                true => field(&data[j as usize])?,
                false => Value::Null,
            }
        }
        WindowOp::Lead => {
            let j = i as f64 + offset;
            match j < n as f64 {
                true => field(&data[j as usize])?,
                false => Value::Null,
            }
        }
        WindowOp::FirstValue => field(&data[w.i0])?,
        WindowOp::LastValue => field(&data[w.i1 - 1])?,
        WindowOp::NthValue => {
            let j = w.i0 as f64 + (window.param - 1.0);
            match j < w.i1 as f64 {
                true => field(&data[j as usize])?,
                false => Value::Null,
            }
        }
        WindowOp::PrevValue => {
            let v = field(&data[i])?;
            if !v.is_nullish() {
                state.prev = v;
            }
            state.prev.clone()
        }
        WindowOp::NextValue => {
            if (i as isize) <= state.next_index {
                state.next.clone()
            } else {
                let mut found = None;
                for (j, t) in data.iter().enumerate().skip(i) {
                    let v = field(t)?;
                    if !v.is_nullish() {
                        found = Some((j, v));
                        break;
                    }
                }
                match found {
                    Some((j, v)) => {
                        state.next_index = j as isize;
                        state.next = v;
                    }
                    None => {
                        state.next_index = n as isize;
                        state.next = Value::Null;
                    }
                }
                state.next.clone()
            }
        }
    })
}
