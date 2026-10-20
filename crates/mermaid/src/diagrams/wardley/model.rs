//! Pele's `diagrams/wardley/model.ts`.

use crate::diagrams::common::db::Titles;
use serde::Serialize;

use super::{
    builder::{
        MapPoint, NewNode, NodeClass, WardleyAnnotation, WardleyBuildResult, WardleyBuilder,
        WardleyForce, WardleyLink, WardleyNote, WardleyTrend,
    },
    parser::{Flow, LabelAst, WardleyAst},
};
use crate::{
    detect::Kind,
    error::Error,
    js::{self},
};

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WardleyModel {
    #[serde(rename = "type")]
    pub kind: &'static str,
    #[serde(flatten)]
    pub titles: Titles,
    #[serde(flatten)]
    pub built: WardleyBuildResult,
}

/// Values up to 1 are fractions; larger ones are already percentages.
fn to_percent(value: f64, context: &str) -> Result<f64, Error> {
    let normalized = if value <= 1.0 { value * 100.0 } else { value };
    if normalized < 0.0 || normalized > 100.0 {
        return Err(Error::semantic(format!(
            "{context} must be between 0-1 (decimal) or 0-100 (percentage). Received: {}",
            js::num_str(value)
        ))
        .of(Kind::Wardley));
    }
    Ok(normalized)
}

fn to_coordinates(visibility: f64, evolution: f64, context: &str) -> Result<MapPoint, Error> {
    let x = to_percent(evolution, &format!("{context} evolution"))?;
    let y = to_percent(visibility, &format!("{context} visibility"))?;
    Ok(MapPoint { x, y })
}

/// The flow and label of a `+'label'>` arrow.
fn flow_from_arrow(arrow: Option<&str>) -> (Option<Flow>, Option<String>) {
    let Some(arrow) = arrow.filter(|arrow| arrow.starts_with('+')) else {
        return (None, None);
    };
    let end = arrow
        .get(2..)
        .and_then(|rest| rest.find('\''))
        .map(|at| at + 2);
    let label = match end {
        Some(end) if arrow.as_bytes().get(1) == Some(&b'\'') => Some(arrow[2..end].to_string()),
        _ => None,
    };
    if arrow.contains("<>") {
        return (Some(Flow::Bidirectional), label);
    }
    if arrow.contains('<') {
        return (Some(Flow::Backward), label);
    }
    if arrow.contains('>') {
        return (Some(Flow::Forward), label);
    }
    (None, label)
}

impl WardleyBuilder {
    /// Mermaid's populateDb: fills the builder from the syntax tree, in
    /// Mermaid's order.
    pub fn populate(&mut self, ast: &WardleyAst) -> Result<(), Error> {
        if let Some((width, height)) = ast.size {
            self.set_size(width, height);
        }

        if let Some(stages) = &ast.evolution {
            self.set_axes(
                stages
                    .iter()
                    .map(|stage| match &stage.second_name {
                        Some(second) if !second.is_empty() => {
                            format!("{} / {}", js::trim(&stage.name), js::trim(second))
                        }
                        _ => js::trim(&stage.name).to_string(),
                    })
                    .collect(),
                stages.iter().filter_map(|stage| stage.boundary).collect(),
            );
        }

        for anchor in &ast.anchors {
            let at = to_coordinates(
                anchor.visibility,
                anchor.evolution,
                &format!("Anchor \"{}\"", anchor.name),
            )?;
            self.add_node(NewNode {
                id: anchor.name.clone(),
                label: anchor.name.clone(),
                at,
                class_name: Some(NodeClass::Anchor),
                label_offset_x: None,
                label_offset_y: None,
                inertia: None,
                source_strategy: None,
            });
        }

        for component in &ast.components {
            let at = to_coordinates(
                component.visibility,
                component.evolution,
                &format!("Component \"{}\"", component.name),
            )?;
            let (label_offset_x, label_offset_y) =
                component.label.as_ref().map(LabelAst::offsets).unzip();
            self.add_node(NewNode {
                id: component.name.clone(),
                label: component.name.clone(),
                at,
                class_name: Some(NodeClass::Component),
                label_offset_x,
                label_offset_y,
                inertia: Some(component.inertia),
                source_strategy: component.strategy,
            });
        }

        for note in &ast.notes {
            let at = to_coordinates(
                note.visibility,
                note.evolution,
                &format!("Note \"{}\"", note.text),
            )?;
            self.add_note(WardleyNote {
                text: note.text.clone(),
                at,
            });
        }

        for pipeline in &ast.pipelines {
            let Some(parent) = self.get_node(&pipeline.parent) else {
                return Err(Error::semantic(format!(
                    "Pipeline \"{}\" must reference an existing component with coordinates.",
                    pipeline.parent
                ))
                .of(Kind::Wardley));
            };
            let y = parent.at.y;
            self.start_pipeline(&pipeline.parent);
            for component in &pipeline.components {
                let id = format!("{}_{}", pipeline.parent, component.name);
                let (label_offset_x, label_offset_y) =
                    component.label.as_ref().map(LabelAst::offsets).unzip();
                let x = to_percent(
                    component.evolution,
                    &format!("Pipeline component \"{}\" evolution", component.name),
                )?;
                self.add_node(NewNode {
                    id: id.clone(),
                    label: component.name.clone(),
                    at: MapPoint { x, y },
                    class_name: Some(NodeClass::PipelineComponent),
                    label_offset_x,
                    label_offset_y,
                    inertia: None,
                    source_strategy: None,
                });
                self.add_pipeline_component(&pipeline.parent, &id);
            }
        }

        for link in &ast.links {
            let dashed = link.arrow.as_deref().is_some_and(|arrow| {
                !arrow.is_empty() && (arrow.contains("-.->") || arrow.contains(".-."))
            });
            let (arrow_flow, arrow_label) = flow_from_arrow(link.arrow.as_deref());
            let flow = link.from_port.or(link.to_port).or(arrow_flow);
            let source = self.resolve_node_id(&link.from);
            let target = self.resolve_node_id(&link.to);
            self.add_link(WardleyLink {
                source,
                target,
                dashed,
                label: arrow_label.or_else(|| link.link_label.clone()),
                flow,
            });
        }

        for evolve in &ast.evolves {
            if let Some(node) = self.get_node(&evolve.component) {
                let y = node.at.y;
                let x = to_percent(
                    evolve.target,
                    &format!("Evolve target for \"{}\"", evolve.component),
                )?;
                self.add_trend(WardleyTrend {
                    node_id: evolve.component.clone(),
                    target: MapPoint { x, y },
                });
            }
        }

        if let Some(first) = ast.annotations.first() {
            self.set_annotations_box(to_coordinates(first.x, first.y, "Annotations box")?);
        }

        for annotation in &ast.annotation {
            let at = to_coordinates(
                annotation.x,
                annotation.y,
                &format!("Annotation {}", js::num_str(annotation.number)),
            )?;
            self.add_annotation(WardleyAnnotation {
                number: annotation.number,
                coordinates: vec![at],
                text: Some(annotation.text.clone()),
            });
        }

        for accelerator in &ast.accelerators {
            let at = to_coordinates(
                accelerator.x,
                accelerator.y,
                &format!("Accelerator \"{}\"", accelerator.name),
            )?;
            self.add_accelerator(WardleyForce {
                name: accelerator.name.clone(),
                at,
            });
        }

        for deaccelerator in &ast.deaccelerators {
            let at = to_coordinates(
                deaccelerator.x,
                deaccelerator.y,
                &format!("Deaccelerator \"{}\"", deaccelerator.name),
            )?;
            self.add_deaccelerator(WardleyForce {
                name: deaccelerator.name.clone(),
                at,
            });
        }
        Ok(())
    }
}

impl WardleyModel {
    pub fn new(ast: WardleyAst, title: Option<&str>) -> Result<Self, Error> {
        let mut db = WardleyBuilder::default();
        db.populate(&ast)?;
        Ok(Self {
            kind: "wardley",
            titles: ast.header.titles(title),
            built: db.build(),
        })
    }
}
