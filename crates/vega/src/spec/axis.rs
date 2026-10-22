use crate::spec::Encode;
use serde::{Deserialize, Serialize};
use serde_json::Value as Json;

macro_rules! axis {
    ($($field:ident: $name:literal),* $(,)?) => {
        /// An axis of `scale` on its `orient` side. Every other property is
        /// read by its Vega name through [`Axis::get`], falling back to the
        /// axis config as the axis parser does.
        #[derive(Clone, Debug, Default, PartialEq, Deserialize, Serialize)]
        #[serde(deny_unknown_fields)]
        pub struct Axis {
            pub scale: String,
            pub orient: Json,
            #[serde(skip_serializing_if = "Option::is_none")]
            pub encode: Option<AxisEncode>,
            $(
                #[serde(rename = $name, skip_serializing_if = "Option::is_none")]
                pub $field: Option<Json>,
            )*
        }

        impl Axis {
            /// The property named `name`, as written.
            pub fn get(&self, name: &str) -> Option<&Json> {
                match name {
                    "orient" => Some(&self.orient),
                    $($name => self.$field.as_ref(),)*
                    _ => None,
                }
            }
        }
    };
}

axis! {
    aria: "aria",
    band_position: "bandPosition",
    description: "description",
    domain: "domain",
    domain_cap: "domainCap",
    domain_color: "domainColor",
    domain_dash: "domainDash",
    domain_dash_offset: "domainDashOffset",
    domain_opacity: "domainOpacity",
    domain_width: "domainWidth",
    format: "format",
    format_type: "formatType",
    grid: "grid",
    grid_cap: "gridCap",
    grid_color: "gridColor",
    grid_dash: "gridDash",
    grid_dash_offset: "gridDashOffset",
    grid_opacity: "gridOpacity",
    grid_scale: "gridScale",
    grid_width: "gridWidth",
    label_align: "labelAlign",
    label_angle: "labelAngle",
    label_baseline: "labelBaseline",
    label_bound: "labelBound",
    label_color: "labelColor",
    label_flush: "labelFlush",
    label_flush_offset: "labelFlushOffset",
    label_font: "labelFont",
    label_font_size: "labelFontSize",
    label_font_style: "labelFontStyle",
    label_font_weight: "labelFontWeight",
    label_limit: "labelLimit",
    label_line_height: "labelLineHeight",
    label_offset: "labelOffset",
    label_opacity: "labelOpacity",
    label_overlap: "labelOverlap",
    label_padding: "labelPadding",
    label_separation: "labelSeparation",
    labels: "labels",
    max_extent: "maxExtent",
    min_extent: "minExtent",
    offset: "offset",
    position: "position",
    tick_band: "tickBand",
    tick_cap: "tickCap",
    tick_color: "tickColor",
    tick_count: "tickCount",
    tick_dash: "tickDash",
    tick_dash_offset: "tickDashOffset",
    tick_extra: "tickExtra",
    tick_min_step: "tickMinStep",
    tick_offset: "tickOffset",
    tick_opacity: "tickOpacity",
    tick_round: "tickRound",
    tick_size: "tickSize",
    tick_width: "tickWidth",
    ticks: "ticks",
    title: "title",
    title_align: "titleAlign",
    title_anchor: "titleAnchor",
    title_angle: "titleAngle",
    title_baseline: "titleBaseline",
    title_color: "titleColor",
    title_font: "titleFont",
    title_font_size: "titleFontSize",
    title_font_style: "titleFontStyle",
    title_font_weight: "titleFontWeight",
    title_limit: "titleLimit",
    title_line_height: "titleLineHeight",
    title_opacity: "titleOpacity",
    title_padding: "titlePadding",
    title_x: "titleX",
    title_y: "titleY",
    translate: "translate",
    values: "values",
    zindex: "zindex",
}

/// An axis's own encoding of its parts.
#[derive(Clone, Debug, Default, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AxisEncode {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub axis: Option<Encode>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ticks: Option<Encode>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub labels: Option<Encode>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub grid: Option<Encode>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub domain: Option<Encode>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<Encode>,
}
