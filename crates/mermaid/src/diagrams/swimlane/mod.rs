//! `swimlane`: a flowchart whose top-level subgraphs are lanes, with the same
//! syntax and the same model. Pele's `diagrams/swimlane`.

use super::{
    Diagram, Model, Render,
    flowchart::{
        FlowDb, Flowchart,
        graph::{FlowGraph, GraphNode},
        render::FlowVariant,
        shapes::NodeShape,
        types::{LabelType, NodeAssets},
    },
};
use serde::Serialize;

use crate::{
    Config, Error, ParseLimits, Rendered,
    geom::{Direction, Point, Size},
    layout::{
        compound::{CEdge, CNode},
        lanes::lane_layout,
        layered::LayeredOptions,
    },
    svg::{
        builder::{EscText, num},
        shapes::Shape,
        theme::ResolvedStyle,
    },
    text::Label,
};
use crate::{Ctx, Host};
use std::fmt::Write as _;

const DEFAULT_LANE: &str = "__swimlane_default__";

struct Lanes;

impl FlowVariant for Lanes {
    fn kind(&self) -> &'static str {
        "swimlane"
    }

    // Nodes outside every lane share one unnamed lane, as in Mermaid. Lanes
    // all run the same way, so a `direction` inside a subgraph has no effect.
    fn prepare(&self, graph: &mut FlowGraph) {
        let mut loose = false;
        for node in &mut graph.nodes {
            if node.is_group {
                node.dir = None;
            } else if node.parent_id.is_none() {
                loose = true;
            }
        }
        if !loose {
            return;
        }
        for node in &mut graph.nodes {
            if !node.is_group && node.parent_id.is_none() {
                node.parent_id = Some(DEFAULT_LANE.to_string());
            }
        }
        graph.nodes.push(GraphNode {
            id: DEFAULT_LANE.to_string(),
            label: None,
            label_type: LabelType::Text,
            shape: NodeShape::Drawn(Shape::Rect),
            is_group: true,
            parent_id: None,
            css_styles: Vec::new(),
            css_compiled_styles: Vec::new(),
            css_classes: String::new(),
            dir: None,
            link: None,
            link_target: None,
            tooltip: None,
            assets: NodeAssets::default(),
        });
    }

    fn layout(
        &self,
        nodes: &mut [CNode],
        edges: &mut [CEdge],
        dir: Direction,
        opt: &LayeredOptions,
        tight: bool,
    ) -> Size {
        lane_layout(
            nodes,
            edges,
            dir,
            opt,
            if tight { Some(12.0) } else { None },
        )
    }

    fn draws_groups(&self) -> bool {
        true
    }

    // A lane: an outline the length of the diagram, with its title in a band
    // at the start.
    fn draw_group(
        &self,
        out: &mut String,
        c: &CNode,
        label: &Label,
        style: &ResolvedStyle,
        classes: &str,
        id: &str,
        dir: Direction,
    ) {
        let p = crate::PREFIX;
        let corner = c.corner();
        let sideways = !dir.is_vertical();
        // The middle of the title band.
        let at = if sideways {
            Point::new(corner.x + c.pad_top / 2.0, c.center.y)
        } else {
            Point::new(c.center.x, corner.y + c.pad_top / 2.0)
        };
        let attrs = format!(
            " class=\"{p}-cluster-label\" fill=\"var(--_m)\"{}",
            style.text
        );
        let text = label.svg(at, &attrs);
        let _ = write!(
            out,
            "<g class=\"{p}-cluster {p}-lane{classes}\" data-id=\"{}\"><rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"none\" stroke=\"var(--_b)\"/><rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"var(--_a)\" stroke=\"var(--_b)\"{}/>",
            EscText(id),
            num(corner.x),
            num(corner.y),
            num(c.size.w),
            num(c.size.h),
            num(corner.x),
            num(corner.y),
            num(if sideways { c.pad_top } else { c.size.w }),
            num(if sideways { c.size.h } else { c.pad_top }),
            style.shape,
        );
        let _ = if sideways && !label.lines.is_empty() {
            write!(
                out,
                "<g transform=\"rotate(-90 {} {})\">{text}</g></g>",
                num(at.x),
                num(at.y)
            )
        } else {
            write!(out, "{text}</g>")
        };
    }
}

pub struct Swimlane;

impl Diagram for Swimlane {
    fn parse(
        &self,
        source: &str,
        config: &Config,
        title: Option<&str>,
        line_offset: usize,
        limits: &ParseLimits,
    ) -> Result<Model, Error> {
        let Model::Flowchart(db) = Flowchart.parse(source, config, title, line_offset, limits)?
        else {
            unreachable!("a flowchart parses to a flowchart model")
        };
        Ok(Model::Swimlane(Box::new(SwimlaneDb(*db))))
    }
}

/// A flowchart drawn with its top-level subgraphs as lanes.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(transparent)]
pub struct SwimlaneDb(pub FlowDb);

impl Render for SwimlaneDb {
    fn render(&self, config: &Config, ctx: &Ctx<'_, impl Host>) -> Result<Rendered, Error> {
        let section = config.swimlane.as_ref().unwrap_or(&config.flowchart);
        Ok(self.0.render_as(section, ctx, Some(&Lanes)))
    }
}
