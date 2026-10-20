//! Pele's `diagrams/wardley/builder.ts`.

use std::collections::HashMap;

use indexmap::IndexMap;
use serde::{Serialize, ser::SerializeMap};

use super::parser::Flow;
use crate::js::{self, JsonNumber};

/// A place on the map, in percent: `x` is evolution, `y` is visibility.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct MapPoint {
    #[serde(serialize_with = "js::json_number")]
    pub x: f64,
    #[serde(serialize_with = "js::json_number")]
    pub y: f64,
}

/// What a node was declared as.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum NodeClass {
    Anchor,
    Component,
    PipelineComponent,
}

/// How a component is sourced.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Strategy {
    Build,
    Buy,
    Outsource,
    Market,
}

impl Strategy {
    /// The strategy a source names in lowercase.
    pub fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "build" => Self::Build,
            "buy" => Self::Buy,
            "outsource" => Self::Outsource,
            "market" => Self::Market,
            _ => return None,
        })
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Build => "build",
            Self::Buy => "buy",
            Self::Outsource => "outsource",
            Self::Market => "market",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct WardleyNode {
    pub id: String,
    pub label: String,
    pub at: MapPoint,
    pub class_name: Option<NodeClass>,
    pub label_offset_x: Option<f64>,
    pub label_offset_y: Option<f64>,
    pub in_pipeline: Option<bool>,
    pub is_pipeline_parent: Option<bool>,
    pub inertia: Option<bool>,
    pub source_strategy: Option<Strategy>,
    /// Whether `is_pipeline_parent` was set before `in_pipeline`, which is
    /// the order the two keys serialize in.
    pub parent_first: bool,
}

impl Serialize for WardleyNode {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let mut map = s.serialize_map(None)?;
        map.serialize_entry("id", &self.id)?;
        map.serialize_entry("label", &self.label)?;
        map.serialize_entry("x", &JsonNumber(self.at.x))?;
        map.serialize_entry("y", &JsonNumber(self.at.y))?;
        if let Some(class_name) = &self.class_name {
            map.serialize_entry("className", class_name)?;
        }
        if let Some(x) = self.label_offset_x {
            map.serialize_entry("labelOffsetX", &JsonNumber(x))?;
        }
        if let Some(y) = self.label_offset_y {
            map.serialize_entry("labelOffsetY", &JsonNumber(y))?;
        }
        if let Some(inertia) = self.inertia {
            map.serialize_entry("inertia", &inertia)?;
        }
        if let Some(strategy) = &self.source_strategy {
            map.serialize_entry("sourceStrategy", strategy)?;
        }
        let parent = self.is_pipeline_parent.map(|v| ("isPipelineParent", v));
        let member = self.in_pipeline.map(|v| ("inPipeline", v));
        let flags = if self.parent_first {
            [parent, member]
        } else {
            [member, parent]
        };
        for (key, value) in flags.into_iter().flatten() {
            map.serialize_entry(key, &value)?;
        }
        map.end()
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct WardleyLink {
    pub source: String,
    pub target: String,
    pub dashed: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub flow: Option<Flow>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct WardleyTrend {
    pub node_id: String,
    pub target: MapPoint,
}

impl Serialize for WardleyTrend {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let mut map = s.serialize_map(Some(3))?;
        map.serialize_entry("nodeId", &self.node_id)?;
        map.serialize_entry("targetX", &JsonNumber(self.target.x))?;
        map.serialize_entry("targetY", &JsonNumber(self.target.y))?;
        map.end()
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WardleyPipeline {
    pub node_id: String,
    pub component_ids: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct WardleyAnnotation {
    #[serde(serialize_with = "js::json_number")]
    pub number: f64,
    pub coordinates: Vec<MapPoint>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct WardleyNote {
    pub text: String,
    #[serde(flatten)]
    pub at: MapPoint,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct WardleyForce {
    pub name: String,
    #[serde(flatten)]
    pub at: MapPoint,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WardleyAxesConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub x_label: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub y_label: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stages: Option<Vec<String>>,
    /// Right edge of each stage, from 0 to 1.
    #[serde(
        skip_serializing_if = "Option::is_none",
        serialize_with = "numbers_opt"
    )]
    pub stage_boundaries: Option<Vec<f64>>,
}

fn numbers_opt<S: serde::Serializer>(xs: &Option<Vec<f64>>, s: S) -> Result<S::Ok, S::Error> {
    match xs {
        Some(xs) => js::json_numbers(xs, s),
        None => s.serialize_unit(),
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct WardleySize {
    #[serde(serialize_with = "js::json_number")]
    pub width: f64,
    #[serde(serialize_with = "js::json_number")]
    pub height: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WardleyBuildResult {
    pub nodes: Vec<WardleyNode>,
    pub links: Vec<WardleyLink>,
    pub trends: Vec<WardleyTrend>,
    pub pipelines: Vec<WardleyPipeline>,
    pub annotations: Vec<WardleyAnnotation>,
    pub notes: Vec<WardleyNote>,
    pub accelerators: Vec<WardleyForce>,
    pub deaccelerators: Vec<WardleyForce>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub annotations_box: Option<MapPoint>,
    pub axes: WardleyAxesConfig,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<WardleySize>,
}

/// A node as `add_node` is given it.
pub struct NewNode {
    pub id: String,
    pub label: String,
    pub at: MapPoint,
    pub class_name: Option<NodeClass>,
    pub label_offset_x: Option<f64>,
    pub label_offset_y: Option<f64>,
    pub inertia: Option<bool>,
    pub source_strategy: Option<Strategy>,
}

#[derive(Default)]
pub struct WardleyBuilder {
    nodes: IndexMap<String, WardleyNode>,
    labels: Option<HashMap<String, String>>,
    links: Vec<WardleyLink>,
    trends: IndexMap<String, WardleyTrend>,
    pipelines: IndexMap<String, WardleyPipeline>,
    annotations: Vec<WardleyAnnotation>,
    notes: Vec<WardleyNote>,
    accelerators: Vec<WardleyForce>,
    deaccelerators: Vec<WardleyForce>,
    annotations_box: Option<MapPoint>,
    axes: WardleyAxesConfig,
    size: Option<WardleySize>,
}

impl WardleyBuilder {
    /// A node added again keeps its class and label offsets unless the new
    /// one sets them.
    pub fn add_node(&mut self, node: NewNode) {
        let merged = match self.nodes.get(&node.id) {
            Some(existing) => WardleyNode {
                id: node.id.clone(),
                label: node.label,
                at: node.at,
                class_name: node.class_name.or(existing.class_name),
                label_offset_x: node.label_offset_x.or(existing.label_offset_x),
                label_offset_y: node.label_offset_y.or(existing.label_offset_y),
                in_pipeline: existing.in_pipeline,
                is_pipeline_parent: existing.is_pipeline_parent,
                inertia: node.inertia,
                source_strategy: node.source_strategy,
                parent_first: existing.parent_first,
            },
            None => WardleyNode {
                id: node.id.clone(),
                label: node.label,
                at: node.at,
                class_name: node.class_name,
                label_offset_x: node.label_offset_x,
                label_offset_y: node.label_offset_y,
                in_pipeline: None,
                is_pipeline_parent: None,
                inertia: node.inertia,
                source_strategy: node.source_strategy,
                parent_first: false,
            },
        };
        self.nodes.insert(node.id, merged);
        self.labels = None;
    }

    pub fn add_link(&mut self, link: WardleyLink) {
        self.links.push(link);
    }

    pub fn add_trend(&mut self, trend: WardleyTrend) {
        self.trends.insert(trend.node_id.clone(), trend);
    }

    pub fn start_pipeline(&mut self, node_id: &str) {
        self.pipelines.insert(
            node_id.to_string(),
            WardleyPipeline {
                node_id: node_id.to_string(),
                component_ids: Vec::new(),
            },
        );
        if let Some(node) = self.nodes.get_mut(node_id) {
            if node.is_pipeline_parent.is_none() && node.in_pipeline.is_none() {
                node.parent_first = true;
            }
            node.is_pipeline_parent = Some(true);
        }
    }

    pub fn add_pipeline_component(&mut self, pipeline_node_id: &str, component_id: &str) {
        if let Some(pipeline) = self.pipelines.get_mut(pipeline_node_id) {
            pipeline.component_ids.push(component_id.to_string());
        }
        if let Some(node) = self.nodes.get_mut(component_id) {
            node.in_pipeline = Some(true);
        }
    }

    pub fn add_annotation(&mut self, annotation: WardleyAnnotation) {
        self.annotations.push(annotation);
    }

    pub fn add_note(&mut self, note: WardleyNote) {
        self.notes.push(note);
    }

    pub fn add_accelerator(&mut self, accelerator: WardleyForce) {
        self.accelerators.push(accelerator);
    }

    pub fn add_deaccelerator(&mut self, deaccelerator: WardleyForce) {
        self.deaccelerators.push(deaccelerator);
    }

    pub fn set_annotations_box(&mut self, at: MapPoint) {
        self.annotations_box = Some(at);
    }

    pub fn set_axes(&mut self, stages: Vec<String>, stage_boundaries: Vec<f64>) {
        self.axes.stages = Some(stages);
        self.axes.stage_boundaries = Some(stage_boundaries);
    }

    pub fn set_size(&mut self, width: f64, height: f64) {
        self.size = Some(WardleySize { width, height });
    }

    pub fn get_node(&self, id: &str) -> Option<&WardleyNode> {
        self.nodes.get(id)
    }

    /// A name is a node id, or else the label of the first node that has it.
    /// Pipeline components have ids like "Parent_Child" and are linked by
    /// label.
    pub fn resolve_node_id(&mut self, name: &str) -> String {
        if self.nodes.contains_key(name) {
            return name.to_string();
        }
        let nodes = &self.nodes;
        let labels = self.labels.get_or_insert_with(|| {
            let mut labels = HashMap::new();
            for (id, node) in nodes {
                labels
                    .entry(node.label.clone())
                    .or_insert_with(|| id.clone());
            }
            labels
        });
        labels
            .get(name)
            .cloned()
            .unwrap_or_else(|| name.to_string())
    }

    pub fn build(self) -> WardleyBuildResult {
        WardleyBuildResult {
            nodes: self.nodes.into_values().collect(),
            links: self.links,
            trends: self.trends.into_values().collect(),
            pipelines: self.pipelines.into_values().collect(),
            annotations: self.annotations,
            notes: self.notes,
            accelerators: self.accelerators,
            deaccelerators: self.deaccelerators,
            annotations_box: self.annotations_box,
            axes: self.axes,
            size: self.size,
        }
    }
}
