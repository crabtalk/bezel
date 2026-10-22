//! vega-parser's default config: what a spec leaves out, by property name.

use serde_json::{Map, Value as Json, json};

/// A config: groups of properties by name, as vega-parser's `config.js`
/// writes its defaults.
#[derive(Clone, Debug)]
pub struct Config(pub Map<String, Json>);

impl Default for Config {
    fn default() -> Self {
        let (font, symbol_size, stroke_width) = ("sans-serif", 30, 2);
        let (color, black, gray, light_gray) = ("#4c78a8", "#000", "#888", "#ddd");
        let Json::Object(config) = json!({
            "description": "Vega visualization",
            "padding": 0,
            "autosize": "pad",
            "background": null,
            "group": null,
            "mark": null,
            "arc": {"fill": color},
            "area": {"fill": color},
            "image": null,
            "line": {"stroke": color, "strokeWidth": stroke_width},
            "path": {"stroke": color},
            "rect": {"fill": color},
            "rule": {"stroke": black},
            "shape": {"stroke": color},
            "symbol": {"fill": color, "size": 64},
            "text": {"fill": black, "font": font, "fontSize": 11},
            "trail": {"fill": color, "size": stroke_width},
            "style": {
                "guide-label": {"fill": black, "font": font, "fontSize": 10},
                "guide-title": {"fill": black, "font": font, "fontSize": 11, "fontWeight": "bold"},
                "group-title": {"fill": black, "font": font, "fontSize": 13, "fontWeight": "bold"},
                "group-subtitle": {"fill": black, "font": font, "fontSize": 12},
                "point": {"size": symbol_size, "strokeWidth": stroke_width, "shape": "circle"},
                "circle": {"size": symbol_size, "strokeWidth": stroke_width},
                "square": {"size": symbol_size, "strokeWidth": stroke_width, "shape": "square"},
                "cell": {"fill": "transparent", "stroke": light_gray},
                "view": {"fill": "transparent"}
            },
            "title": {"orient": "top", "anchor": "middle", "offset": 4, "subtitlePadding": 3},
            "axis": {
                "minExtent": 0,
                "maxExtent": 200,
                "bandPosition": 0.5,
                "domain": true,
                "domainWidth": 1,
                "domainColor": gray,
                "grid": false,
                "gridWidth": 1,
                "gridColor": light_gray,
                "labels": true,
                "labelAngle": 0,
                "labelLimit": 180,
                "labelOffset": 0,
                "labelPadding": 2,
                "ticks": true,
                "tickColor": gray,
                "tickOffset": 0,
                "tickRound": true,
                "tickSize": 5,
                "tickWidth": 1,
                "titlePadding": 4
            },
            "axisBand": {"tickOffset": -0.5},
            "projection": {"type": "mercator"},
            "legend": {
                "orient": "right",
                "padding": 0,
                "gridAlign": "each",
                "columnPadding": 10,
                "rowPadding": 2,
                "symbolDirection": "vertical",
                "gradientDirection": "vertical",
                "gradientLength": 200,
                "gradientThickness": 16,
                "gradientStrokeColor": light_gray,
                "gradientStrokeWidth": 0,
                "gradientLabelOffset": 2,
                "labelAlign": "left",
                "labelBaseline": "middle",
                "labelLimit": 160,
                "labelOffset": 4,
                "labelOverlap": true,
                "symbolLimit": 30,
                "symbolType": "circle",
                "symbolSize": 100,
                "symbolOffset": 0,
                "symbolStrokeWidth": 1.5,
                "symbolBaseFillColor": "transparent",
                "symbolBaseStrokeColor": gray,
                "titleLimit": 180,
                "titleOrient": "top",
                "titlePadding": 5,
                "layout": {
                    "offset": 18,
                    "direction": "horizontal",
                    "left": {"direction": "vertical"},
                    "right": {"direction": "vertical"}
                }
            },
            "range": {
                "category": {"scheme": "tableau10"},
                "ordinal": {"scheme": "blues"},
                "heatmap": {"scheme": "yellowgreenblue"},
                "ramp": {"scheme": "blues"},
                "diverging": {"scheme": "blueorange", "extent": [1, 0]},
                "symbol": [
                    "circle", "square", "triangle-up", "cross",
                    "diamond", "triangle-right", "triangle-down", "triangle-left"
                ]
            }
        }) else {
            unreachable!("an object literal")
        };
        Self(config)
    }
}

impl Config {
    /// The group of properties named `name`, empty where the config has
    /// none.
    pub fn group(&self, name: &str) -> &Map<String, Json> {
        static EMPTY: std::sync::OnceLock<Map<String, Json>> = std::sync::OnceLock::new();
        match self.0.get(name) {
            Some(Json::Object(group)) => group,
            _ => EMPTY.get_or_init(Map::new),
        }
    }

    /// The style named `name`, empty where the config has none.
    pub fn style(&self, name: &str) -> &Map<String, Json> {
        static EMPTY: std::sync::OnceLock<Map<String, Json>> = std::sync::OnceLock::new();
        match self.group("style").get(name) {
            Some(Json::Object(style)) => style,
            _ => EMPTY.get_or_init(Map::new),
        }
    }
}
