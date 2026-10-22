//! Vega-Lite's default config, by property name.

use serde_json::{Map, Value as Json, json};

/// A config: groups of properties by name, as Vega-Lite's `defaultConfig`
/// writes its defaults.
#[derive(Clone, Debug)]
pub struct Config(pub Map<String, Json>);

impl Default for Config {
    fn default() -> Self {
        let rect = json!({
            "binSpacing": 0,
            "continuousBandSize": 5,
            "minBandSize": 0.25,
            "timeUnitBandPosition": 0.5
        });
        let mut bar = rect.clone();
        bar["binSpacing"] = json!(1);
        let Json::Object(config) = json!({
            "background": "white",
            "padding": 5,
            "timeFormat": "%b %d, %Y",
            "countTitle": "Count of Records",
            "view": {"continuousWidth": 300, "continuousHeight": 300, "step": 20},
            "mark": {"color": "#4c78a8", "invalid": "break-paths-show-path-domains", "timeUnitBandSize": 1},
            "bar": bar,
            "rect": rect,
            "scale": {
                "pointPadding": 0.5,
                "barBandPaddingInner": 0.1,
                "rectBandPaddingInner": 0,
                "tickBandPaddingInner": 0.25,
                "bandWithNestedOffsetPaddingInner": 0.2,
                "bandWithNestedOffsetPaddingOuter": 0.2,
                "minBandSize": 2,
                "minFontSize": 8,
                "maxFontSize": 40,
                "minOpacity": 0.3,
                "maxOpacity": 0.8,
                "minSize": 4,
                "minStrokeWidth": 1,
                "maxStrokeWidth": 4,
                "quantileCount": 4,
                "quantizeCount": 4,
                "zero": true,
                "framesPerSecond": 2,
                "animationDuration": 5
            },
            "style": {},
            "normalizedNumberFormat": ".0%"
        }) else {
            unreachable!("an object literal")
        };
        Self(config)
    }
}

impl Config {
    /// The property `name` of the group `group`.
    pub fn get(&self, group: &str, name: &str) -> Option<&Json> {
        self.0.get(group)?.get(name).filter(|v| !v.is_null())
    }

    /// `getStyleConfig`: the last of `styles` to set `name`.
    pub fn style(&self, name: &str, styles: &[String]) -> Option<&Json> {
        styles
            .iter()
            .filter_map(|style| self.get("style", style)?.get(name))
            .next_back()
    }

    /// `getMarkConfig`: a mark property from its style, else the config of
    /// its type, else the general mark config; `vg_channel` read first where
    /// the Vega channel differs.
    pub fn mark(
        &self,
        name: &str,
        mark: &str,
        styles: &[String],
        vg_channel: Option<&str>,
    ) -> Option<&Json> {
        let style = self.style(name, styles);
        vg_channel
            .and(style)
            .or(style)
            .or_else(|| vg_channel.and_then(|c| self.get(mark, c)))
            .or_else(|| self.get(mark, name))
            .or_else(|| match vg_channel {
                Some(c) => self.get("mark", c),
                None => self.get("mark", name),
            })
    }
}
