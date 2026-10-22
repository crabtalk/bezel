//! Vega-Lite's `normalizeBoxPlot`: a box from each group's first to third
//! quartile with a tick at its median, and whiskers to each group's least
//! and greatest value (`extent: "min-max"`), or to those within `extent`
//! interquartile ranges of the box, the values beyond drawn as outliers.

use crate::{
    transform::{Aggregate, Op},
    vega_lite::{
        composite::{self, Extract, Factory, Unit},
        dataflow::Step,
        format,
        view::{Body, View},
    },
};
use js::number::Number as JsNumber;
use serde_json::{Map, Value, json};

impl Unit {
    pub(crate) fn boxplot(&self) -> Option<View> {
        let Value::Object(config) = json!({
            "size": 14,
            "extent": 1.5,
            "box": {},
            "median": {"color": "white"},
            "outliers": {},
            "rule": {},
            "ticks": null,
        }) else {
            unreachable!("an object")
        };
        let extent = self.def.get("extent").unwrap_or(&config["extent"]);
        // The whiskers' reach in interquartile ranges, `None` for min-max.
        let tukey = match extent {
            Value::Number(k) => Some(JsNumber(k.as_f64()?)),
            Value::String(minmax) if minmax == "min-max" => None,
            _ => return None,
        };
        let size = self.def.get("size").unwrap_or(&config["size"]).clone();
        let size = format::js(&size).truthy().then_some(size);
        let invalid = self.def.get("invalid");

        let orient = self.orient()?;
        let axis = composite::axis(orient);
        let continuous = self.continuous_def(axis)?;
        let field = continuous.get("field")?.as_str()?.to_string();
        let aliased = composite::flat(&field)?;
        let (min, max) = match tukey {
            Some(_) => ("min", "max"),
            None => ("lower_whisker", "upper_whisker"),
        };
        let quartiles = [
            composite::measure(Op::Q1, &field, format!("lower_box_{aliased}")),
            composite::measure(Op::Q3, &field, format!("upper_box_{aliased}")),
        ];
        let mut specific = quartiles.to_vec();
        specific.extend([
            composite::measure(Op::Median, &field, format!("mid_box_{aliased}")),
            composite::measure(Op::Min, &field, format!("{min}_{aliased}")),
            composite::measure(Op::Max, &field, format!("{max}_{aliased}")),
        ]);
        let rest = composite::without(&self.encoding, &[axis]);
        let extract = Extract::of(&composite::without_plain_tooltip(&rest))?;
        let mut boxes = extract.steps();
        let mut measures = extract.measures.clone();
        measures.extend(specific);
        boxes.push(composite::aggregate(measures, &extract.groupby));

        let encoding = composite::without(&extract.encoding, &["tooltip"]);
        let plain = composite::without(&encoding, &["color", "size"]);
        let mut midtick = plain.clone();
        if let Some(size) = encoding
            .get("size")
            .filter(|size| format::js(size).truthy())
        {
            midtick.insert("size".into(), size.clone());
        }
        let parts = Factory {
            unit: self,
            config: &config,
            axis,
            continuous: &continuous,
        };
        let mark = |value: Value| -> Map<String, Value> {
            let Value::Object(mut mark) = value else {
                unreachable!("an object")
            };
            if let Some(invalid) = invalid {
                mark.insert("invalid".into(), invalid.clone());
            }
            mark
        };
        let rule = || mark(json!({"type": "rule", "aria": false}));
        let tick = || {
            mark(json!({
                "type": "tick",
                "color": "black",
                "opacity": 1,
                "orient": composite::across(orient).to_string(),
                "aria": false,
            }))
        };
        let whiskers: Vec<View> = [
            parts.part("rule", rule(), "lower_whisker", Some("lower_box"), &plain)?,
            parts.part("rule", rule(), "upper_box", Some("upper_whisker"), &plain)?,
            parts.part("ticks", tick(), "lower_whisker", None, &plain)?,
            parts.part("ticks", tick(), "upper_whisker", None, &plain)?,
        ]
        .into_iter()
        .flatten()
        .collect();
        let mut bar = mark(json!({"type": "bar"}));
        if let Some(size) = &size {
            bar.insert("size".into(), size.clone());
        }
        bar.insert("orient".into(), orient.to_string().into());
        bar.insert("ariaRoleDescription".into(), "box".into());
        let mut median = mark(json!({"type": "tick"}));
        if let Some(color) = config["median"].get("color") {
            median.insert("color".into(), color.clone());
        }
        if let Some(size) = &size {
            median.insert("size".into(), size.clone());
        }
        median.insert(
            "orient".into(),
            composite::across(orient).to_string().into(),
        );
        median.insert("aria".into(), false.into());
        let mut box_layers = match tukey {
            Some(_) => Vec::new(),
            None => whiskers.clone(),
        };
        box_layers.extend(parts.part("box", bar, "lower_box", Some("upper_box"), &encoding)?);
        box_layers.extend(parts.part("median", median, "mid_box", None, &midtick)?);

        let Some(k) = tukey else {
            let mut transform = self.transform.clone();
            transform.extend(boxes);
            return Some(View {
                data: self.data.clone(),
                transform,
                body: Body::Layer(box_layers),
            });
        };
        let joined = [format!("lower_box_{field}"), format!("upper_box_{field}")];
        let [lower_box, upper_box] = joined.clone().map(|field| composite::datum(&field));
        let iqr = format!("({upper_box} - {lower_box})");
        let lower_whisker = format!("{lower_box} - {k} * {iqr}");
        let upper_whisker = format!("{upper_box} + {k} * {iqr}");
        let value = composite::datum(&field);
        let join = Step::JoinAggregate(Aggregate {
            groupby: extract.groupby.clone(),
            measures: quartiles.to_vec(),
        });
        let mut within = vec![
            composite::measure(Op::Min, &field, format!("lower_whisker_{aliased}")),
            composite::measure(Op::Max, &field, format!("upper_whisker_{aliased}")),
            composite::measure(
                Op::Min,
                &format!("lower_box_{field}"),
                format!("lower_box_{aliased}"),
            ),
            composite::measure(
                Op::Max,
                &format!("upper_box_{field}"),
                format!("upper_box_{aliased}"),
            ),
        ];
        within.extend(extract.measures.clone());
        let mut whisker_view = View {
            data: None,
            transform: vec![
                composite::filter(
                    &format!("({lower_whisker} <= {value}) && ({value} <= {upper_whisker})"),
                    &joined,
                )?,
                composite::aggregate(within, &extract.groupby),
            ],
            body: Body::Layer(whiskers),
        };

        let mut position = Map::new();
        position.insert("field".into(), field.clone().into());
        if let Some(kind) = continuous.get("type") {
            position.insert("type".into(), kind.clone());
        }
        position.insert("title".into(), composite::title(&continuous)?);
        for key in ["scale", "axis"] {
            if let Some(value) = continuous.get(key) {
                position.insert(key.into(), value.clone());
            }
        }
        let mut points = Map::new();
        points.insert(axis.into(), Value::Object(position));
        points.extend(plain);
        if let Some(color) = encoding
            .get("color")
            .filter(|color| format::js(color).truthy())
        {
            points.insert("color".into(), color.clone());
        }
        let outliers = self.part(
            "outliers",
            &config,
            Map::from_iter([("type".to_string(), Value::from("point"))]),
            points,
            vec![composite::filter(
                &format!("({value} < {lower_whisker}) || ({value} > {upper_whisker})"),
                &joined,
            )?],
        );
        let mut joined = extract.steps();
        joined.push(join);
        let filtered = match outliers {
            Some(outliers) => View {
                data: None,
                transform: joined,
                body: Body::Layer(vec![outliers, whisker_view]),
            },
            None => {
                joined.append(&mut whisker_view.transform);
                whisker_view.transform = joined;
                whisker_view
            }
        };
        Some(View {
            data: self.data.clone(),
            transform: self.transform.clone(),
            body: Body::Layer(vec![
                filtered,
                View {
                    data: None,
                    transform: boxes,
                    body: Body::Layer(box_layers),
                },
            ]),
        })
    }
}
