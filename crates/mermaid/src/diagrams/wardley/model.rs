//! Pele's `diagrams/wardley/model.ts`.

use serde::Serialize;

use super::{
    builder::{
        NewNode, WardleyAnnotation, WardleyBuildResult, WardleyBuilder, WardleyForce, WardleyLink,
        WardleyNote, WardleyTrend,
    },
    parser::{LabelAst, WardleyAst},
};
use crate::{
    detect::Kind,
    error::Error,
    js::{self, Lazy},
};

static RE_LEAD: Lazy = Lazy::new(r"^\s+", "g");
static RE_INDENT: Lazy = Lazy::new(r"\n\s+", "g");

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WardleyModel {
    #[serde(rename = "type")]
    pub kind: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub acc_title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub acc_descr: Option<String>,
    #[serde(flatten)]
    pub built: WardleyBuildResult,
}

fn invalid(message: String) -> Error {
    Error::semantic(message).of(Kind::Wardley)
}

/// Values up to 1 are fractions; larger ones are already percentages.
fn to_percent(value: f64, context: &str) -> Result<f64, Error> {
    let normalized = if value <= 1.0 { value * 100.0 } else { value };
    if normalized < 0.0 || normalized > 100.0 {
        return Err(invalid(format!(
            "{context} must be between 0-1 (decimal) or 0-100 (percentage). Received: {}",
            js::num_str(value)
        )));
    }
    Ok(normalized)
}

fn to_coordinates(visibility: f64, evolution: f64, context: &str) -> Result<(f64, f64), Error> {
    let x = to_percent(evolution, &format!("{context} evolution"))?;
    let y = to_percent(visibility, &format!("{context} visibility"))?;
    Ok((x, y))
}

fn flow_from_port(port: Option<&str>) -> Option<&'static str> {
    match port {
        Some("+<>") => Some("bidirectional"),
        Some("+<") => Some("backward"),
        Some("+>") => Some("forward"),
        _ => None,
    }
}

fn flow_from_arrow(arrow: Option<&str>) -> (Option<&'static str>, Option<String>) {
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
        return (Some("bidirectional"), label);
    }
    if arrow.contains('<') {
        return (Some("backward"), label);
    }
    if arrow.contains('>') {
        return (Some("forward"), label);
    }
    (None, label)
}

fn offsets(label: Option<&LabelAst>) -> (Option<f64>, Option<f64>) {
    match label {
        Some(label) => (
            Some((if label.neg_x { -1.0 } else { 1.0 }) * label.offset_x),
            Some((if label.neg_y { -1.0 } else { 1.0 }) * label.offset_y),
        ),
        None => (None, None),
    }
}

/// Mermaid's populateDb: fills the builder from the syntax tree, in
/// Mermaid's order.
pub fn populate(ast: &WardleyAst, db: &mut WardleyBuilder) -> Result<(), Error> {
    if let Some((width, height)) = ast.size {
        db.set_size(width, height);
    }

    if let Some(stages) = &ast.evolution {
        db.set_axes(
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
        let (x, y) = to_coordinates(
            anchor.visibility,
            anchor.evolution,
            &format!("Anchor \"{}\"", anchor.name),
        )?;
        db.add_node(NewNode {
            id: anchor.name.clone(),
            label: anchor.name.clone(),
            x,
            y,
            class_name: Some("anchor".into()),
            label_offset_x: None,
            label_offset_y: None,
            inertia: None,
            source_strategy: None,
        });
    }

    for component in &ast.components {
        let (x, y) = to_coordinates(
            component.visibility,
            component.evolution,
            &format!("Component \"{}\"", component.name),
        )?;
        let (label_offset_x, label_offset_y) = offsets(component.label.as_ref());
        db.add_node(NewNode {
            id: component.name.clone(),
            label: component.name.clone(),
            x,
            y,
            class_name: Some("component".into()),
            label_offset_x,
            label_offset_y,
            inertia: Some(component.inertia),
            source_strategy: component.strategy.clone(),
        });
    }

    for note in &ast.notes {
        let (x, y) = to_coordinates(
            note.visibility,
            note.evolution,
            &format!("Note \"{}\"", note.text),
        )?;
        db.add_note(WardleyNote {
            text: note.text.clone(),
            x,
            y,
        });
    }

    for pipeline in &ast.pipelines {
        let Some(parent) = db.get_node(&pipeline.parent) else {
            return Err(invalid(format!(
                "Pipeline \"{}\" must reference an existing component with coordinates.",
                pipeline.parent
            )));
        };
        let y = parent.y;
        db.start_pipeline(&pipeline.parent);
        for component in &pipeline.components {
            let id = format!("{}_{}", pipeline.parent, component.name);
            let (label_offset_x, label_offset_y) = offsets(component.label.as_ref());
            let x = to_percent(
                component.evolution,
                &format!("Pipeline component \"{}\" evolution", component.name),
            )?;
            db.add_node(NewNode {
                id: id.clone(),
                label: component.name.clone(),
                x,
                y,
                class_name: Some("pipeline-component".into()),
                label_offset_x,
                label_offset_y,
                inertia: None,
                source_strategy: None,
            });
            db.add_pipeline_component(&pipeline.parent, &id);
        }
    }

    for link in &ast.links {
        let dashed = link.arrow.as_deref().is_some_and(|arrow| {
            !arrow.is_empty() && (arrow.contains("-.->") || arrow.contains(".-."))
        });
        let (arrow_flow, arrow_label) = flow_from_arrow(link.arrow.as_deref());
        let flow = flow_from_port(link.from_port.as_deref())
            .or_else(|| flow_from_port(link.to_port.as_deref()))
            .or(arrow_flow);
        let source = db.resolve_node_id(&link.from);
        let target = db.resolve_node_id(&link.to);
        db.add_link(WardleyLink {
            source,
            target,
            dashed,
            label: arrow_label.or_else(|| link.link_label.clone()),
            flow,
        });
    }

    for evolve in &ast.evolves {
        if let Some(node) = db.get_node(&evolve.component) {
            let target_y = node.y;
            let target_x = to_percent(
                evolve.target,
                &format!("Evolve target for \"{}\"", evolve.component),
            )?;
            db.add_trend(WardleyTrend {
                node_id: evolve.component.clone(),
                target_x,
                target_y,
            });
        }
    }

    if let Some(first) = ast.annotations.first() {
        let (x, y) = to_coordinates(first.x, first.y, "Annotations box")?;
        db.set_annotations_box(x, y);
    }

    for annotation in &ast.annotation {
        let (x, y) = to_coordinates(
            annotation.x,
            annotation.y,
            &format!("Annotation {}", js::num_str(annotation.number)),
        )?;
        db.add_annotation(WardleyAnnotation {
            number: annotation.number,
            coordinates: vec![super::builder::Point { x, y }],
            text: Some(annotation.text.clone()),
        });
    }

    for accelerator in &ast.accelerators {
        let (x, y) = to_coordinates(
            accelerator.x,
            accelerator.y,
            &format!("Accelerator \"{}\"", accelerator.name),
        )?;
        db.add_accelerator(WardleyForce {
            name: accelerator.name.clone(),
            x,
            y,
        });
    }

    for deaccelerator in &ast.deaccelerators {
        let (x, y) = to_coordinates(
            deaccelerator.x,
            deaccelerator.y,
            &format!("Deaccelerator \"{}\"", deaccelerator.name),
        )?;
        db.add_deaccelerator(WardleyForce {
            name: deaccelerator.name.clone(),
            x,
            y,
        });
    }
    Ok(())
}

pub fn build_wardley(ast: WardleyAst, title: Option<&str>) -> Result<WardleyModel, Error> {
    let mut db = WardleyBuilder::default();
    populate(&ast, &mut db)?;
    Ok(WardleyModel {
        kind: "wardley",
        title: ast
            .title
            .filter(|title| !title.is_empty())
            .or_else(|| title.map(str::to_string)),
        acc_title: ast
            .acc_title
            .filter(|text| !text.is_empty())
            .map(|text| RE_LEAD.replace(&text, |_| String::new())),
        acc_descr: ast
            .acc_descr
            .filter(|text| !text.is_empty())
            .map(|text| RE_INDENT.replace(&text, |_| "\n".into())),
        built: db.build(),
    })
}
