//! `swimlane`: a flowchart whose top-level subgraphs are lanes, with the same
//! syntax and the same model. Pele's `diagrams/swimlane`.

use super::{
    Diagram, Model, Render,
    flowchart::{
        FlowDb, Flowchart,
        graph::{FlowGraph, GraphNode},
        render::{FlowVariant, render_flowchart},
        types::LabelType,
    },
};
use serde::Serialize;

use crate::geom::{Direction, Point, Size};
use crate::{
    Config, Error, ParseLimits, Rendered,
    layout::{
        compound::{CEdge, CNode},
        lanes::lane_layout,
        layered::LayeredOptions,
    },
    svg::{
        builder::{EscText, num},
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
            shape: "rect".to_string(),
            is_group: true,
            parent_id: None,
            css_styles: Vec::new(),
            css_compiled_styles: Vec::new(),
            css_classes: String::new(),
            dir: None,
            link: None,
            link_target: None,
            tooltip: None,
            icon: None,
            pos: None,
            img: None,
            asset_width: None,
            asset_height: None,
            constraint: None,
            color_index: None,
            metadata: None,
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
        let left = c.center.x - c.size.w / 2.0;
        let top = c.center.y - c.size.h / 2.0;
        let sideways = !dir.is_vertical();
        let cx = if sideways {
            left + c.pad_top / 2.0
        } else {
            c.center.x
        };
        let cy = if sideways {
            c.center.y
        } else {
            top + c.pad_top / 2.0
        };
        let attrs = format!(
            " class=\"{p}-cluster-label\" fill=\"var(--_m)\"{}",
            style.text
        );
        let text = label.svg(Point::new(cx, cy), &attrs);
        let _ = write!(
            out,
            "<g class=\"{p}-cluster {p}-lane{classes}\" data-id=\"{}\"><rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"none\" stroke=\"var(--_b)\"/><rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"var(--_a)\" stroke=\"var(--_b)\"{}/>",
            EscText(id),
            num(left),
            num(top),
            num(c.size.w),
            num(c.size.h),
            num(left),
            num(top),
            num(if sideways { c.pad_top } else { c.size.w }),
            num(if sideways { c.size.h } else { c.pad_top }),
            style.shape,
        );
        let _ = if sideways && !label.lines.is_empty() {
            write!(
                out,
                "<g transform=\"rotate(-90 {} {})\">{text}</g></g>",
                num(cx),
                num(cy)
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
        Ok(render_flowchart(&self.0, section, ctx, Some(&Lanes)))
    }
}
