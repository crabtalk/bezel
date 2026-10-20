//! A diagram's configuration: Mermaid's config object as front matter,
//! `%%{init}%%` directives and [`crate::Options::config`] give it, merged as
//! JSON and read into [`Config`]. A value of the wrong type reads as unset.

use serde::Deserialize;
pub use serde_json::Value;
use serde_with::{DefaultOnError, serde_as};

use crate::detect::Kind;
use crate::diagrams::agentflow::AgentflowConfig;
use crate::diagrams::architecture::ArchitectureConfig;
use crate::diagrams::block::BlockConfig;
use crate::diagrams::class::ClassConfig;
use crate::diagrams::cynefin::CynefinConfig;
use crate::diagrams::flowchart::FlowchartConfig;
use crate::diagrams::gantt::GanttConfig;
use crate::diagrams::git::GitGraphConfig;
use crate::diagrams::kanban::KanbanConfig;
use crate::diagrams::mindmap::MindmapConfig;
use crate::diagrams::packet::PacketConfig;
use crate::diagrams::pie::PieConfig;
use crate::diagrams::quadrant::QuadrantConfig;
use crate::diagrams::radar::RadarConfig;
use crate::diagrams::railroad::RailroadConfig;
use crate::diagrams::sankey::SankeyConfig;
use crate::diagrams::sequence::SequenceConfig;
use crate::diagrams::timeline::TimelineConfig;
use crate::diagrams::treemap::TreemapConfig;
use crate::diagrams::treeview::TreeViewConfig;
use crate::diagrams::usecase::UsecaseConfig;
use crate::diagrams::wardley::WardleyConfig;
use crate::diagrams::xychart::XyChartConfig;

/// A JSON object, keys in the order they were written.
pub type Map = serde_json::Map<String, Value>;

/// Mermaid's deep merge of `source` into `target`: objects merge, anything
/// else replaces, and the keys that reach an object's prototype are dropped.
pub fn merge(target: &mut Map, source: &Map) {
    for (key, value) in source {
        if matches!(key.as_str(), "__proto__" | "constructor" | "prototype") {
            continue;
        }
        match (value, target.get_mut(key)) {
            (Value::Object(from), Some(Value::Object(current))) => merge(current, from),
            (Value::Object(from), _) => {
                let mut fresh = Map::new();
                merge(&mut fresh, from);
                target.insert(key.clone(), Value::Object(fresh));
            }
            (other, _) => {
                target.insert(key.clone(), other.clone());
            }
        }
    }
}

/// Mermaid's config: the keys diagrams read from the top level, and each
/// diagram type's section.
#[serde_as]
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Config {
    #[serde_as(as = "DefaultOnError")]
    pub wrap: Option<bool>,
    #[serde_as(as = "DefaultOnError")]
    pub direction: Option<String>,
    #[serde_as(as = "DefaultOnError")]
    pub wrapping_width: Option<f64>,
    #[serde_as(as = "DefaultOnError")]
    pub node_spacing: Option<f64>,
    #[serde_as(as = "DefaultOnError")]
    pub rank_spacing: Option<f64>,
    #[serde_as(as = "DefaultOnError")]
    pub flowchart: FlowchartConfig,
    #[serde_as(as = "DefaultOnError")]
    pub swimlane: Option<FlowchartConfig>,
    #[serde_as(as = "DefaultOnError")]
    pub sequence: SequenceConfig,
    #[serde_as(as = "DefaultOnError")]
    pub class: ClassConfig,
    #[serde_as(as = "DefaultOnError")]
    pub state: BaseConfig,
    #[serde_as(as = "DefaultOnError")]
    pub er: BaseConfig,
    #[serde_as(as = "DefaultOnError")]
    pub pie: PieConfig,
    #[serde_as(as = "DefaultOnError")]
    pub gantt: GanttConfig,
    #[serde(rename = "gitGraph")]
    #[serde_as(as = "DefaultOnError")]
    pub git_graph: GitGraphConfig,
    #[serde_as(as = "DefaultOnError")]
    pub mindmap: MindmapConfig,
    #[serde_as(as = "DefaultOnError")]
    pub timeline: TimelineConfig,
    #[serde_as(as = "DefaultOnError")]
    pub journey: BaseConfig,
    #[serde(rename = "xyChart")]
    #[serde_as(as = "DefaultOnError")]
    pub xy_chart: XyChartConfig,
    #[serde(rename = "quadrantChart")]
    #[serde_as(as = "DefaultOnError")]
    pub quadrant_chart: QuadrantConfig,
    #[serde_as(as = "DefaultOnError")]
    pub requirement: BaseConfig,
    #[serde_as(as = "DefaultOnError")]
    pub c4: BaseConfig,
    #[serde_as(as = "DefaultOnError")]
    pub sankey: SankeyConfig,
    #[serde_as(as = "DefaultOnError")]
    pub block: BlockConfig,
    #[serde_as(as = "DefaultOnError")]
    pub packet: PacketConfig,
    #[serde_as(as = "DefaultOnError")]
    pub kanban: KanbanConfig,
    #[serde_as(as = "DefaultOnError")]
    pub architecture: ArchitectureConfig,
    #[serde_as(as = "DefaultOnError")]
    pub radar: RadarConfig,
    #[serde_as(as = "DefaultOnError")]
    pub treemap: TreemapConfig,
    #[serde_as(as = "DefaultOnError")]
    pub info: BaseConfig,
    #[serde_as(as = "DefaultOnError")]
    pub agentflow: AgentflowConfig,
    #[serde_as(as = "DefaultOnError")]
    pub usecase: UsecaseConfig,
    #[serde_as(as = "DefaultOnError")]
    pub railroad: RailroadConfig,
    #[serde(rename = "treeView")]
    #[serde_as(as = "DefaultOnError")]
    pub tree_view: TreeViewConfig,
    #[serde_as(as = "DefaultOnError")]
    pub venn: BaseConfig,
    #[serde_as(as = "DefaultOnError")]
    pub cynefin: CynefinConfig,
    #[serde(rename = "wardley-beta")]
    #[serde_as(as = "DefaultOnError")]
    pub wardley: WardleyConfig,
    #[serde_as(as = "DefaultOnError")]
    pub ishikawa: BaseConfig,
    #[serde_as(as = "DefaultOnError")]
    pub eventmodeling: BaseConfig,
}

impl Config {
    /// `map` read as config.
    pub fn new(map: Map) -> Self {
        if map.is_empty() {
            return Self::default();
        }
        Self::deserialize(Value::Object(map)).unwrap_or_default()
    }

    /// What every type's section holds, for type `kind`.
    pub fn base(&self, kind: Kind) -> Option<&BaseConfig> {
        match kind {
            Kind::Flowchart => Some(&self.flowchart.base),
            Kind::Swimlane => self.swimlane.as_ref().map(|s| &s.base),
            Kind::Sequence => Some(&self.sequence.base),
            Kind::Class => Some(&self.class.base),
            Kind::State => Some(&self.state),
            Kind::Er => Some(&self.er),
            Kind::Pie => Some(&self.pie.base),
            Kind::Gantt => Some(&self.gantt.base),
            Kind::GitGraph => Some(&self.git_graph.base),
            Kind::Mindmap => Some(&self.mindmap.base),
            Kind::Timeline => Some(&self.timeline.base),
            Kind::Journey => Some(&self.journey),
            Kind::XyChart => Some(&self.xy_chart.base),
            Kind::QuadrantChart => Some(&self.quadrant_chart.base),
            Kind::Requirement => Some(&self.requirement),
            Kind::C4 => Some(&self.c4),
            Kind::Sankey => Some(&self.sankey.base),
            Kind::Block => Some(&self.block.base),
            Kind::Packet => Some(&self.packet.base),
            Kind::Kanban => Some(&self.kanban.base),
            Kind::Architecture => Some(&self.architecture.base),
            Kind::Radar => Some(&self.radar.base),
            Kind::Treemap => Some(&self.treemap.base),
            Kind::Info => Some(&self.info),
            Kind::Agentflow => Some(&self.agentflow.base),
            Kind::Usecase => Some(&self.usecase.base),
            Kind::Railroad => Some(&self.railroad.base),
            Kind::TreeView => Some(&self.tree_view.base),
            Kind::Venn => Some(&self.venn),
            Kind::Cynefin => Some(&self.cynefin.base),
            Kind::Wardley => Some(&self.wardley.base),
            Kind::Ishikawa => Some(&self.ishikawa),
            Kind::Eventmodeling => Some(&self.eventmodeling),
        }
    }
}

/// What every diagram type's section may hold.
#[serde_as]
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct BaseConfig {
    #[serde_as(as = "DefaultOnError")]
    pub use_max_width: Option<bool>,
}

/// `value` when it is positive, at most 2000, or `fallback`.
pub(crate) fn positive(value: Option<f64>, fallback: f64) -> f64 {
    match value {
        Some(value) if value > 0.0 => value.min(2000.0),
        _ => fallback,
    }
}

/// `value` when it is at least `min`, capped at `max`, or `fallback`.
pub(crate) fn capped(value: Option<f64>, fallback: f64, min: f64, max: f64) -> f64 {
    match value {
        Some(value) if value >= min => value.min(max),
        _ => fallback,
    }
}

/// `value` when it lies in `min..=max`, or `fallback`.
pub(crate) fn within(value: Option<f64>, fallback: f64, min: f64, max: f64) -> f64 {
    match value {
        Some(value) if value >= min && value <= max => value,
        _ => fallback,
    }
}

/// `value` held to `min..=max` when it is a finite number.
pub(crate) fn clamped(value: Option<f64>, min: f64, max: f64) -> Option<f64> {
    value
        .filter(|value| value.is_finite())
        .map(|value| max.min(min.max(value)))
}

/// `value` when it is at least `min`, capped at `max`.
pub(crate) fn capped_option(value: Option<f64>, min: f64, max: f64) -> Option<f64> {
    value
        .filter(|&value| value >= min)
        .map(|value| value.min(max))
}
