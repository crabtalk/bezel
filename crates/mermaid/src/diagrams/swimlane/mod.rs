//! `swimlane`: a flowchart whose top-level subgraphs are lanes, with the same
//! syntax and the same model. Pele's `diagrams/swimlane`.

use super::{
    Diagram, Model,
    flowchart::{
        Flowchart,
        graph::{FlowGraph, GraphNode},
        render::{FlowVariant, render_flowchart},
        types::LabelType,
    },
};
use crate::{
    Config, Error, ParseLimits, Rendered,
    layout::{
        compound::{CEdge, CNode, CompoundResult, Dir},
        lanes::lane_layout,
        layered::LayeredOptions,
    },
    svg::{
        builder::{EscText, LabelSvg, num},
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
        dir: Dir,
        opt: &LayeredOptions,
        tight: bool,
    ) -> CompoundResult {
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
        dir: Dir,
    ) {
        let p = crate::PREFIX;
        let left = c.x - c.w / 2.0;
        let top = c.y - c.h / 2.0;
        let sideways = dir == Dir::LR || dir == Dir::RL;
        let cx = if sideways {
            left + c.pad_top / 2.0
        } else {
            c.x
        };
        let cy = if sideways { c.y } else { top + c.pad_top / 2.0 };
        let attrs = format!(
            " class=\"{p}-cluster-label\" fill=\"var(--_m)\"{}",
            style.text
        );
        let text = LabelSvg(label, cx, cy, &attrs);
        let _ = write!(
            out,
            "<g class=\"{p}-cluster {p}-lane{classes}\" data-id=\"{}\"><rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"none\" stroke=\"var(--_b)\"/><rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"var(--_a)\" stroke=\"var(--_b)\"{}/>",
            EscText(id),
            num(left),
            num(top),
            num(c.w),
            num(c.h),
            num(left),
            num(top),
            num(if sideways { c.pad_top } else { c.w }),
            num(if sideways { c.h } else { c.pad_top }),
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
        Flowchart.parse(source, config, title, line_offset, limits)
    }
}

pub fn render(model: &Model, config: &Config, ctx: &Ctx<'_, impl Host>) -> Result<Rendered, Error> {
    let Model::Flowchart(db) = model else {
        unreachable!("swimlane renders a flowchart model")
    };
    Ok(render_flowchart(db, config, ctx, Some(&Lanes)))
}
