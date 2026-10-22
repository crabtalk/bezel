//! Vega-Lite's `normalizeErrorBand`: an error bar's bounds as a band, an
//! area with `interpolate` and `tension` over both axes or a rect over one,
//! with a line or rule along each bound as `borders`.

use crate::vega_lite::{
    composite::{Factory, Unit},
    format,
    view::{Body, View},
};
use serde_json::{Map, Value, json};

impl Unit {
    pub(crate) fn errorband(&self) -> Option<View> {
        let Value::Object(config) = json!({"band": {"opacity": 0.3}, "borders": false}) else {
            unreachable!("an object")
        };
        // The center Vega-Lite falls back to is the error bar's.
        let Value::Object(errorbar) = json!({"center": "mean"}) else {
            unreachable!("an object")
        };
        let params = self.error_params(&errorbar)?;
        let parts = Factory {
            unit: self,
            config: &config,
            axis: params.axis,
            continuous: &params.continuous,
        };
        let both = self.encoding.contains_key("x") && self.encoding.contains_key("y");
        let mut band = Map::new();
        let mut borders = Map::new();
        band.insert("type".into(), if both { "area" } else { "rect" }.into());
        borders.insert("type".into(), if both { "line" } else { "rule" }.into());
        if both {
            let interpolate = self
                .def
                .get("interpolate")
                .filter(|value| format::js(value).truthy());
            if let Some(interpolate) = interpolate {
                let tension = self
                    .def
                    .get("tension")
                    .filter(|value| format::js(value).truthy());
                for mark in [&mut band, &mut borders] {
                    mark.insert("interpolate".into(), interpolate.clone());
                    if let Some(tension) = tension {
                        mark.insert("tension".into(), tension.clone());
                    }
                }
            }
            band.insert("ariaRoleDescription".into(), "errorband".into());
            borders.insert("aria".into(), false.into());
        }
        let shared = &params.encoding;
        let layers: Vec<View> = [
            parts.part("band", band, "lower", Some("upper"), shared)?,
            parts.part("borders", borders.clone(), "lower", None, shared)?,
            parts.part("borders", borders, "upper", None, shared)?,
        ]
        .into_iter()
        .flatten()
        .collect();
        Some(View {
            data: self.data.clone(),
            transform: params.transform,
            body: Body::Layer(layers),
        })
    }
}
