//! Vega-Lite's dataflow: the nodes each view's rows pass through, from a
//! source to each unit's marks, as Vega-Lite parses them from the views and
//! its optimizer reshapes them, run in the order Vega evaluates them.
//!
//! A layer's units start from what its own transforms make of its rows, or
//! from a source of their own data; views with the same inline values, url
//! or dataset name and format share one source. A unit in a layer bins its
//! encoding's fields before its own transforms, and otherwise after them;
//! then come its encoding's time units, the rows its scales sort by, its
//! encoding's aggregate and the rows its marks draw. The tree holds no
//! parse, identifier, invalid filter, impute or stack nodes.

use crate::{
    data::Data,
    model::Kind,
    transform::{Bins, Random, Transform},
    vega_lite::{
        sort,
        unit::Unit,
        view::{Body, View},
    },
};
use gpui::SharedString;
pub(crate) use node::{Formula, Node, Step};
use serde_json::Value;

mod node;
mod optimize;

/// The nodes and how they link, as Vega-Lite's tree of them: a node keeps
/// its children in order, and a node taken out keeps the children it had.
#[derive(Default)]
pub(crate) struct Tree {
    slots: Vec<Slot>,
    roots: Vec<usize>,
    /// The nodes whose rows units read, moved as the optimizer merges
    /// nodes away.
    taps: Vec<usize>,
}

struct Slot {
    parent: Option<usize>,
    children: Vec<usize>,
    node: Node,
}

/// A source: its `data`, and every field the units under it read, with the
/// type each reads it as.
pub(crate) struct Source<'v> {
    pub(crate) data: &'v Value,
    pub(crate) reads: Vec<(SharedString, Option<Kind>)>,
}

/// What a tree ran: each node's rows, and the edges of each bin it
/// computed.
pub(crate) struct Ran {
    rows: Vec<Option<Data>>,
    edges: Vec<Vec<(SharedString, Bins)>>,
}

impl Tree {
    /// The tree of `view`'s units, as Vega-Lite parses it before optimizing,
    /// with its sources and units, each unit's taps in [`Unit::raw`] and
    /// [`Unit::main`], `config` the spec's. `None` for a unit this does not
    /// read or a view with no data.
    pub(crate) fn parse<'v>(
        view: &'v View,
        config: Option<&Value>,
    ) -> Option<(Self, Vec<Source<'v>>, Vec<Unit>)> {
        let mut parse = Parse {
            config,
            tree: Self::default(),
            sources: Vec::new(),
            units: Vec::new(),
        };
        parse.view(view, None, false)?;
        Some((parse.tree, parse.sources, parse.units))
    }

    /// The rows the nodes under the roots make, each node after its parent
    /// and the nodes in the order Vega evaluates them, `table` giving each
    /// source's rows. `None` for a source `table` has no rows of.
    pub(crate) fn run(
        &self,
        table: &mut dyn FnMut(usize) -> Option<Data>,
        random: &mut Random,
    ) -> Option<Ran> {
        let mut ran = Ran {
            rows: vec![None; self.slots.len()],
            edges: vec![Vec::new(); self.slots.len()],
        };
        for &root in &self.roots {
            let Node::Source(source) = self.slots[root].node else {
                continue;
            };
            let mut stack = vec![(root, table(source)?)];
            while let Some((node, rows)) = stack.pop() {
                let (rows, edges) = self.slots[node].node.apply(&rows, random);
                for &child in self.slots[node].children.iter().rev() {
                    stack.push((child, rows.clone()));
                }
                ran.rows[node] = Some(rows);
                ran.edges[node] = edges;
            }
        }
        Some(ran)
    }

    /// The node a tap reads.
    pub(crate) fn tap(&self, tap: usize) -> usize {
        self.taps[tap]
    }

    /// A new node under `parent`, the last of its children.
    fn add(&mut self, parent: Option<usize>, node: Node) -> usize {
        let id = self.slots.len();
        self.slots.push(Slot {
            parent: None,
            children: Vec::new(),
            node,
        });
        match parent {
            Some(parent) => self.set_parent(id, parent),
            None => self.roots.push(id),
        }
        id
    }

    /// A new tap on `node`.
    fn tap_on(&mut self, node: usize) -> usize {
        self.taps.push(node);
        self.taps.len() - 1
    }
}

impl Ran {
    /// The rows a node makes.
    pub(crate) fn rows(&self, node: usize) -> Option<&Data> {
        self.rows[node].as_ref()
    }

    /// The edges of the bin whose starts are `field`, as the nearest bin
    /// node over `node` computed them.
    pub(crate) fn bins(&self, tree: &Tree, node: usize, field: &str) -> Option<Bins> {
        let mut at = Some(node);
        while let Some(node) = at {
            if let Some((_, bins)) = self.edges[node].iter().find(|(start, _)| start == field) {
                return Some(*bins);
            }
            at = tree.slots[node].parent;
        }
        None
    }
}

/// Vega-Lite's `parseData` over a tree of views.
struct Parse<'v, 'c> {
    config: Option<&'c Value>,
    tree: Tree,
    sources: Vec<Source<'v>>,
    units: Vec<Unit>,
}

impl<'v> Parse<'v, '_> {
    /// `view`'s nodes under `parent`, the head of its parent layer and the
    /// source that head reads, a layer's before its children's. A layer with
    /// no data passes none to its children.
    fn view(
        &mut self,
        view: &'v View,
        parent: Option<(usize, usize)>,
        in_layer: bool,
    ) -> Option<()> {
        let (mut head, source) = match (&view.data, parent, &view.body) {
            (Some(data), ..) => self.source(data),
            (None, Some(parent), _) => parent,
            (None, None, Body::Layer(children)) => {
                for child in children {
                    self.view(child, None, true)?;
                }
                return Some(());
            }
            (None, None, Body::Unit { .. }) => return None,
        };
        let steps = |parse: &mut Self, mut head: usize| {
            for step in &view.transform {
                if let Step::Transform(Transform::TimeUnit(unit)) = step {
                    parse.sources[source]
                        .reads
                        .push((unit.field.clone(), Some(Kind::Temporal)));
                }
                if let Some(node) = Node::of(step) {
                    head = parse.tree.add(Some(head), node);
                }
            }
            head
        };
        match &view.body {
            Body::Layer(children) => {
                head = steps(self, head);
                for child in children {
                    self.view(child, Some((head, source)), true)?;
                }
            }
            Body::Unit { mark, encoding } => {
                let mut unit = Unit::read(mark, encoding, self.config)?;
                let bins = Node::bins(&unit.defs);
                if in_layer && let Some(bins) = bins.clone() {
                    head = self.tree.add(Some(head), bins);
                }
                head = steps(self, head);
                if !in_layer && let Some(bins) = bins {
                    head = self.tree.add(Some(head), bins);
                }
                if let Some(units) = Node::time_units(&unit.defs) {
                    head = self.tree.add(Some(head), units);
                }
                // Scales that sort by an op read the rows before the
                // aggregate.
                if unit.defs.iter().any(sort::by_op) {
                    head = self.tree.add(Some(head), Node::Output);
                }
                unit.raw = self.tree.tap_on(head);
                if let Some(aggregate) = Node::encoding_aggregate(&unit.defs) {
                    head = self.tree.add(Some(head), aggregate);
                }
                head = self.tree.add(Some(head), Node::Output);
                unit.main = self.tree.tap_on(head);
                unit.source = source;
                self.sources[source].reads.extend(unit.reads());
                self.units.push(unit);
            }
        }
        Some(())
    }

    /// The source node of `data`, and its source: the one made already of
    /// the same inline values, url or dataset name in the same format.
    fn source(&mut self, data: &'v Value) -> (usize, usize) {
        let key = |data: &Value| -> Option<(&str, Value)> {
            let data = data.as_object()?;
            let format = data.get("format").cloned().unwrap_or_default();
            ["values", "url", "name"]
                .into_iter()
                .find_map(|key| Some((key, data.get(key)?.clone())))
                .map(|(key, value)| (key, Value::Array(vec![value, format])))
        };
        let found = key(data).and_then(|key_of| {
            self.sources
                .iter()
                .position(|source| key(source.data).as_ref() == Some(&key_of))
        });
        let index = match found {
            Some(index) => index,
            None => {
                self.sources.push(Source {
                    data,
                    reads: Vec::new(),
                });
                self.sources.len() - 1
            }
        };
        let node = match self
            .tree
            .slots
            .iter()
            .position(|slot| matches!(slot.node, Node::Source(s) if s == index))
        {
            Some(node) => node,
            None => self.tree.add(None, Node::Source(index)),
        };
        (node, index)
    }
}
