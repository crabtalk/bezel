//! `compile/data`: the unit's dataflow, from its source through the stack
//! and the invalid-value filter, and the Vega data sets it assembles to.

use crate::{
    Error,
    compile::{Channel, Unit, field, scale::Component},
    spec::Type,
};
use vega::spec as vg;

/// The assembled data sets, and the name of the one the marks read.
pub struct Data {
    pub assembled: Vec<vg::Data>,
    pub main: String,
}

/// `parseData` and `assembleRootData` for a unit.
pub fn parse(unit: &Unit, scales: &[Component]) -> Result<Data, Error> {
    let data = unit
        .spec
        .data
        .as_ref()
        .ok_or_else(|| Error::Unsupported("a unit without data".into()))?;
    if data.url.is_some() {
        return Err(Error::Unsupported("url data".into()));
    }
    let source = vg::Data {
        name: "source_0".into(),
        values: data.values.clone(),
        ..vg::Data::default()
    };
    let mut transform = Vec::new();
    if let Some(stack) = &unit.stack {
        let def = unit
            .field_def(stack.field_channel)
            .expect("a stacked field");
        let name = |suffix| {
            field::vg_field(
                def,
                &field::Options {
                    suffix: Some(suffix),
                    ..Default::default()
                },
            )
        };
        transform.push(vg::Transform::Stack(vg::Stack {
            groupby: stack
                .groupby_fields
                .iter()
                .cloned()
                .map(vg::Field::Name)
                .collect(),
            field: Some(vg::Field::Name(field::vg_field(
                def,
                &field::Options::default(),
            ))),
            sort: Some(vg::Compare {
                field: Some(vg::Many::Many(Vec::new())),
                order: Some(vg::Many::Many(Vec::new())),
            }),
            as_: Some(vg::Param::Value([name("start"), name("end")])),
            offset: Some(vg::Param::Value(stack.offset)),
            signal: None,
        }));
    }
    // `FilterInvalidNode`: a mark's invalid mode is `filter` unless it is a
    // path, so a field on a continuous scale drops its invalid values.
    let mut filters: Vec<String> = Vec::new();
    for channel in [Channel::X, Channel::Y] {
        let (Some(def), Some(scale)) = (
            unit.field_def(channel),
            scales.iter().find(|s| s.channel == channel),
        ) else {
            continue;
        };
        if scale.is_discrete() {
            continue;
        }
        let reference = field::vg_field(
            def,
            &field::Options {
                expr: Some("datum"),
                ..Default::default()
            },
        );
        match def.type_ {
            Type::Quantitative => {
                filters.push(format!("isValid({reference}) && isFinite(+{reference})"))
            }
            Type::Temporal => filters.push(format!(
                "(isDate({reference}) || (isValid({reference}) && isFinite(+{reference})))"
            )),
            _ => {}
        }
    }
    if !filters.is_empty() {
        transform.push(vg::Transform::Filter(vg::Filter {
            expr: filters.join(" && "),
            signal: None,
        }));
    }
    let main = if transform.is_empty() {
        source.name.clone()
    } else {
        "data_0".to_string()
    };
    let mut assembled = vec![source];
    if !transform.is_empty() {
        assembled.push(vg::Data {
            name: main.clone(),
            source: Some(vg::Many::One("source_0".into())),
            transform,
            ..vg::Data::default()
        });
    }
    Ok(Data { assembled, main })
}
