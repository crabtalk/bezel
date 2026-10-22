//! vega-transforms' `TimeUnit`.

use crate::{
    Error,
    spec::Timeunit,
    time::{self, Floor},
    transform::{Accessor, Tuple, or},
    value::Value,
    view::View,
};

/// Each tuple's field floored to the units, and where that step ends.
pub fn timeunit(view: &View, def: &Timeunit, mut tuples: Vec<Tuple>) -> Result<Vec<Tuple>, Error> {
    let field = Accessor::new(view, &def.field)?;
    let band = or(view, &def.interval, true)?;
    let utc = or(view, &def.timezone, "local".to_string())? == "utc";
    let infer = or(view, &def.infer_units, false)?;
    let (units, step) = match def.units.as_ref().map(|u| view.param(u)).transpose()? {
        _ if infer => {
            let times = tuples
                .iter()
                .map(|t| field.get(view, &t.datum).map(|v| expr_time(&v)))
                .collect::<Result<Vec<_>, _>>()?;
            let (units, step) = time::detect(&times, utc)?;
            let names: Vec<String> = units.iter().map(|u| u.name().to_string()).collect();
            (time::units(&names)?, step)
        }
        Some(units) => (time::units(&units)?, or(view, &def.step, 1.0)?),
        None => {
            let extent = match def.extent.as_ref().map(|e| view.param(e)).transpose()? {
                Some(extent) => [0, 1].map(|i| {
                    extent
                        .get(i)
                        .map_or(f64::NAN, |v| Value::from(v).to_number())
                }),
                None => extent(view, &field, &tuples)?,
            };
            let (units, step) = time::bin(extent, or(view, &def.maxbins, 40.0)?);
            (
                time::units(
                    &units
                        .iter()
                        .map(|u| u.name().to_string())
                        .collect::<Vec<_>>(),
                )?,
                step,
            )
        }
    };
    let step = if step == 0.0 || step.is_nan() {
        1.0
    } else {
        step
    };
    let last = *units.last().expect("at least one unit");
    let floor = Floor::new(units, step, utc);
    let interval = last.interval(floor.zone());
    let [u0, u1] = or(view, &def.as_, ["unit0".into(), "unit1".into()])?;
    for t in &mut tuples {
        let v = field.get(view, &t.datum)?;
        if v.is_nullish() {
            t.datum = t.datum.with_field(&u0, Value::Null);
            if band {
                t.datum = t.datum.with_field(&u1, Value::Null);
            }
            continue;
        }
        let a = floor.floor(v.to_number());
        t.datum = t.datum.with_field(&u0, Value::Number(a));
        if band {
            let mut end = floor.zone().date(a);
            interval.offset(&mut end, step.floor());
            t.datum = t.datum.with_field(&u1, Value::Number(end.time()));
        }
    }
    Ok(tuples)
}

/// `+new Date(value)`: a string read in the host's zone.
fn expr_time(value: &Value) -> f64 {
    match value {
        Value::String(s) => d3::time::date::LocalZone::default().parse(s).time(),
        v => v.to_number(),
    }
}

/// vega-util's `extent` of the field: the least and greatest values but
/// null and `NaN`.
fn extent(view: &View, field: &Accessor, tuples: &[Tuple]) -> Result<[f64; 2], Error> {
    let (mut min, mut max) = (f64::NAN, f64::NAN);
    for t in tuples {
        let v = field.get(view, &t.datum)?;
        if v.is_nullish() {
            continue;
        }
        let v = v.to_number();
        if v.is_nan() {
            continue;
        }
        if min.is_nan() || v < min {
            min = v;
        }
        if max.is_nan() || v > max {
            max = v;
        }
    }
    Ok([min, max])
}
