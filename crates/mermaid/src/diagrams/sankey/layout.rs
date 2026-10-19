//! Pele's `diagrams/sankey/layout.ts`.

use std::cmp::Ordering;

use super::db::SankeyModel;
use crate::{detect::Kind, error::Error};

const PASSES: i32 = 6;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Alignment {
    Justify,
    Left,
    Right,
    Center,
}

pub struct LayoutOptions {
    pub width: f64,
    pub height: f64,
    pub node_width: f64,
    pub node_padding: f64,
    /// Height of a label line; thin nodes are kept at least this far apart
    /// where there is room.
    pub label_height: f64,
    pub alignment: Alignment,
}

pub struct PlacedNode {
    pub layer: usize,
    /// Total flow through the node, in the diagram's units.
    pub value: f64,
    pub x: f64,
    pub y: f64,
    pub height: f64,
}

pub struct PlacedLink {
    pub source: usize,
    pub target: usize,
    pub value: f64,
    pub thickness: f64,
    /// Vertical centers of the band where it leaves its source and where it
    /// enters its target.
    pub source_y: f64,
    pub target_y: f64,
}

pub struct SankeyLayout {
    pub nodes: Vec<PlacedNode>,
    pub links: Vec<PlacedLink>,
}

/// `compare`'s answer as an ordering: below zero is less, `NaN` equal.
fn ordering(x: f64) -> Ordering {
    x.partial_cmp(&0.0).unwrap_or(Ordering::Equal)
}

/// Pele's sort: an insertion sort for a short list, the built-in one for a
/// longer.
fn sort(list: &mut [usize], compare: impl Fn(usize, usize) -> f64) {
    if list.len() > 8 {
        list.sort_by(|&a, &b| ordering(compare(a, b)));
        return;
    }
    for i in 1..list.len() {
        let item = list[i];
        let mut j = i;
        while j > 0 && compare(list[j - 1], item) > 0.0 {
            list[j] = list[j - 1];
            j -= 1;
        }
        list[j] = item;
    }
}

/// `diff || a - b`.
fn or_index(diff: f64, a: usize, b: usize) -> f64 {
    if diff != 0.0 && !diff.is_nan() {
        diff
    } else {
        a as f64 - b as f64
    }
}

/// Places nodes in columns by their distance from a source, sizes them by
/// their flow, and then lets each column settle next to its neighbours so
/// that the bands cross less.
pub fn layout_sankey(model: &SankeyModel, options: &LayoutOptions) -> Result<SankeyLayout, Error> {
    let n = model.nodes.len();
    let mut links: Vec<PlacedLink> = model
        .links
        .iter()
        .map(|link| PlacedLink {
            source: link.source.index,
            target: link.target.index,
            value: if link.value.is_finite() && link.value > 0.0 {
                link.value
            } else {
                0.0
            },
            thickness: 0.0,
            source_y: 0.0,
            target_y: 0.0,
        })
        .collect();
    let mut outgoing: Vec<Vec<usize>> = vec![Vec::new(); n];
    let mut incoming: Vec<Vec<usize>> = vec![Vec::new(); n];
    for (k, link) in links.iter().enumerate() {
        outgoing[link.source].push(k);
        incoming[link.target].push(k);
    }

    // Longest distance from a source, in topological order; what is left
    // over lies on a cycle.
    let mut depth = vec![0i32; n];
    let mut pending = vec![0i32; n];
    let mut order: Vec<usize> = Vec::new();
    for i in 0..n {
        pending[i] = incoming[i].len() as i32;
        if pending[i] == 0 {
            order.push(i);
        }
    }
    let mut q = 0;
    while q < order.len() {
        let i = order[q];
        for &k in &outgoing[i] {
            let t = links[k].target;
            if depth[t] < depth[i] + 1 {
                depth[t] = depth[i] + 1;
            }
            pending[t] -= 1;
            if pending[t] == 0 {
                order.push(t);
            }
        }
        q += 1;
    }
    if order.len() < n {
        // Every node left over has a predecessor that is left over too;
        // walking back finds the loop.
        let mut stuck = 0;
        while pending[stuck] == 0 {
            stuck += 1;
        }
        let mut seen = vec![false; n];
        while !seen[stuck] {
            seen[stuck] = true;
            for &k in &incoming[stuck] {
                if pending[links[k].source] > 0 {
                    stuck = links[k].source;
                    break;
                }
            }
        }
        return Err(Error::semantic(format!(
            "Sankey diagrams cannot have circular links, and \"{}\" is part of one.",
            model.nodes[stuck].id
        ))
        .of(Kind::Sankey));
    }
    let mut layers: i32 = 1;
    for &d in &depth {
        if d + 1 > layers {
            layers = d + 1;
        }
    }
    let mut reach = vec![0i32; n];
    for q in (0..n).rev() {
        let i = order[q];
        for &k in &outgoing[i] {
            if reach[i] < reach[links[k].target] + 1 {
                reach[i] = reach[links[k].target] + 1;
            }
        }
    }

    // Flows are scaled to the largest one, so that sums of huge values stay
    // finite.
    let mut largest = 0.0;
    for link in &links {
        if link.value > largest {
            largest = link.value;
        }
    }
    let unit = if largest > 0.0 { 1.0 / largest } else { 0.0 };

    let mut nodes: Vec<PlacedNode> = Vec::with_capacity(n);
    let mut flow = vec![0f64; n];
    let mut columns: Vec<Vec<usize>> = vec![Vec::new(); layers as usize];
    let step = if layers > 1 {
        (options.width - options.node_width) / (layers - 1) as f64
    } else {
        0.0
    };
    for i in 0..n {
        let layer = match options.alignment {
            Alignment::Left => depth[i],
            Alignment::Right => layers - 1 - reach[i],
            Alignment::Center => {
                let mut layer = depth[i];
                if incoming[i].is_empty() && !outgoing[i].is_empty() {
                    layer = layers;
                    for &k in &outgoing[i] {
                        if depth[links[k].target] - 1 < layer {
                            layer = depth[links[k].target] - 1;
                        }
                    }
                }
                layer
            }
            Alignment::Justify => {
                if !outgoing[i].is_empty() {
                    depth[i]
                } else {
                    layers - 1
                }
            }
        } as usize;
        let mut into = 0.0;
        let mut out = 0.0;
        let mut value = 0.0;
        let mut value_out = 0.0;
        for &k in &incoming[i] {
            into += links[k].value * unit;
            value += links[k].value;
        }
        for &k in &outgoing[i] {
            out += links[k].value * unit;
            value_out += links[k].value;
        }
        flow[i] = f64::max(into, out);
        nodes.push(PlacedNode {
            layer,
            value: f64::max(value, value_out),
            x: layer as f64 * step,
            y: 0.0,
            height: 0.0,
        });
        columns[layer].push(i);
    }

    let mut tallest = 1;
    for column in &columns {
        if column.len() > tallest {
            tallest = column.len();
        }
    }
    let gap = if tallest > 1 {
        options
            .node_padding
            .min(options.height / (tallest - 1) as f64)
    } else {
        options.node_padding
    };
    let mut scale = f64::INFINITY;
    for column in &columns {
        let mut sum = 0.0;
        for &i in column {
            sum += flow[i];
        }
        if sum > 0.0 {
            scale = scale.min((options.height - (column.len() as f64 - 1.0) * gap) / sum);
        }
    }
    if !scale.is_finite() || scale < 0.0 {
        scale = 0.0;
    }

    // A thin node still needs room for its label. Where every column can
    // afford it, each node claims at least a label's height, and the flows
    // are scaled down to make that fit.
    let mut claim = f64::max(0.0, options.label_height - gap);
    let fits = |s: f64, claim: f64| -> bool {
        for column in &columns {
            let mut extent = (column.len() as f64 - 1.0) * gap;
            for &i in column {
                extent += f64::max(flow[i] * s, claim);
            }
            if extent > options.height + 1e-6 {
                return false;
            }
        }
        true
    };
    if claim > 0.0 && !fits(scale, claim) {
        if fits(0.0, claim) {
            let mut low = 0.0;
            for _ in 0..40 {
                let mid = (low + scale) / 2.0;
                if fits(mid, claim) {
                    low = mid;
                } else {
                    scale = mid;
                }
            }
            scale = low;
        } else {
            claim = 0.0;
        }
    }

    // The room a node keeps clear above and below itself.
    let mut margin = vec![0f64; n];
    for link in &mut links {
        link.thickness = link.value * unit * scale;
    }
    for column in &columns {
        let mut y = 0.0;
        for &i in column {
            nodes[i].height = flow[i] * scale;
            margin[i] = f64::max(0.0, claim - nodes[i].height) / 2.0;
            nodes[i].y = y + margin[i];
            y += nodes[i].height + 2.0 * margin[i] + gap;
        }
        // Spare room is shared out between the nodes of a column.
        let spare = (options.height - y + gap) / (column.len() as f64 + 1.0);
        for (k, &i) in column.iter().enumerate() {
            nodes[i].y += spare * (k as f64 + 1.0);
        }
    }

    let mut state = Settle {
        nodes,
        links,
        outgoing,
        incoming,
        margin,
        gap,
        height: options.height,
        source_offset: vec![0.0; model.links.len()],
        target_offset: vec![0.0; model.links.len()],
    };

    state.stack_all();
    for pass in 0..PASSES {
        let pull = 0.99f64.powf(f64::from(pass));
        for l in (0..(layers - 1).max(0) as usize).rev() {
            state.settle(&mut columns[l], pull, false);
        }
        state.stack_all();
        for column in columns.iter_mut().skip(1) {
            state.settle(column, pull, true);
        }
        state.stack_all();
    }
    let Settle {
        nodes,
        mut links,
        source_offset,
        target_offset,
        ..
    } = state;
    for (k, link) in links.iter_mut().enumerate() {
        link.source_y = nodes[link.source].y + source_offset[k];
        link.target_y = nodes[link.target].y + target_offset[k];
    }

    Ok(SankeyLayout { nodes, links })
}

struct Settle {
    nodes: Vec<PlacedNode>,
    links: Vec<PlacedLink>,
    outgoing: Vec<Vec<usize>>,
    incoming: Vec<Vec<usize>>,
    margin: Vec<f64>,
    gap: f64,
    height: f64,
    /// Bands leave and enter a node in the order of where they go, so they do
    /// not cross at it. These are the offsets of each band's middle from the
    /// top of the node at either end.
    source_offset: Vec<f64>,
    target_offset: Vec<f64>,
}

impl Settle {
    fn center(nodes: &[PlacedNode], i: usize) -> f64 {
        nodes[i].y + nodes[i].height / 2.0
    }

    fn stack_out(&mut self, i: usize) {
        let (nodes, links) = (&self.nodes, &self.links);
        sort(&mut self.outgoing[i], |a, b| {
            or_index(
                Self::center(nodes, links[a].target) - Self::center(nodes, links[b].target),
                a,
                b,
            )
        });
        let mut y = 0.0;
        for &k in &self.outgoing[i] {
            self.source_offset[k] = y + self.links[k].thickness / 2.0;
            y += self.links[k].thickness;
        }
    }

    fn stack_in(&mut self, i: usize) {
        let (nodes, links) = (&self.nodes, &self.links);
        sort(&mut self.incoming[i], |a, b| {
            or_index(
                Self::center(nodes, links[a].source) - Self::center(nodes, links[b].source),
                a,
                b,
            )
        });
        let mut y = 0.0;
        for &k in &self.incoming[i] {
            self.target_offset[k] = y + self.links[k].thickness / 2.0;
            y += self.links[k].thickness;
        }
    }

    fn stack_all(&mut self) {
        for i in 0..self.nodes.len() {
            self.stack_out(i);
            self.stack_in(i);
        }
    }

    /// Moves each node of a column towards where its bands would run level,
    /// then restores the order and the gaps. `forward` looks at the bands
    /// coming in, otherwise at those going out.
    fn settle(&mut self, column: &mut [usize], pull: f64, forward: bool) {
        for &i in column.iter() {
            let empty = if forward {
                self.incoming[i].is_empty()
            } else {
                self.outgoing[i].is_empty()
            };
            if empty {
                continue;
            }
            if forward {
                self.stack_in(i);
            } else {
                self.stack_out(i);
            }
            let list = if forward {
                &self.incoming[i]
            } else {
                &self.outgoing[i]
            };
            let mut weighted = 0.0;
            let mut weight = 0.0;
            let mut plain = 0.0;
            for &k in list {
                let link = &self.links[k];
                let level = if forward {
                    self.nodes[link.source].y + self.source_offset[k] - self.target_offset[k]
                } else {
                    self.nodes[link.target].y + self.target_offset[k] - self.source_offset[k]
                };
                weighted += level * link.thickness;
                weight += link.thickness;
                plain += Self::center(&self.nodes, if forward { link.source } else { link.target })
                    - self.nodes[i].height / 2.0;
            }
            let wanted = if weight > 0.0 {
                weighted / weight
            } else {
                plain / list.len() as f64
            };
            self.nodes[i].y += (wanted - self.nodes[i].y) * pull;
        }
        let nodes = &self.nodes;
        sort(column, |a, b| {
            or_index(Self::center(nodes, a) - Self::center(nodes, b), a, b)
        });
        let mut y = 0.0;
        for &i in column.iter() {
            if self.nodes[i].y - self.margin[i] < y {
                self.nodes[i].y = y + self.margin[i];
            }
            y = self.nodes[i].y + self.nodes[i].height + self.margin[i] + self.gap;
        }
        y = self.height;
        for &i in column.iter().rev() {
            if self.nodes[i].y + self.nodes[i].height + self.margin[i] > y {
                self.nodes[i].y = y - self.nodes[i].height - self.margin[i];
            }
            y = self.nodes[i].y - self.margin[i] - self.gap;
        }
    }
}
