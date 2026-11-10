use crate::{
    change::Change,
    contain,
    edit::{self, CanvasEditor, Item},
    kind, mindmap,
    model::{self, End, Node, TextMode},
};

impl CanvasEditor {
    /// Mint the node's id and place its top-left corner at `at`.
    pub fn add_node(&mut self, node: Node, at: (i64, i64)) -> Option<String> {
        let root = mindmap::root(self.canvas(), node, at);
        self.added(Some(vec![root]))
    }

    pub fn group_selected(&mut self, label: &str, padding: i64) -> Option<String> {
        let ids = self.selected_nodes();
        let (left, top, right, bottom) =
            edit::extent(ids.iter().filter_map(|id| self.canvas().node(id)))?;
        let id = self.canvas().mint();
        let padding = padding.max(0);
        let group = Node {
            id: id.clone(),
            kind: model::GROUP.into(),
            x: left - padding,
            y: top - padding,
            width: right - left + padding * 2,
            height: bottom - top + padding * 2,
            label: Some(label.to_owned()),
            ..Node::default()
        };
        let mut changes = vec![Change::AddNode {
            node: group,
            index: Some(0),
        }];
        for member in &ids {
            if ids.iter().any(|other| {
                other != member
                    && self
                        .contents(&[(*other).to_owned()])
                        .iter()
                        .any(|inner| inner == member)
            }) {
                continue;
            }
            if let Some(change) = contain::hold(self.canvas(), member, Some(&id)) {
                changes.push(change);
            }
        }
        self.added(Some(changes))
    }

    pub fn set_selection_color(&mut self, color: Option<String>) {
        let changes = self
            .selection()
            .iter()
            .filter_map(|item| match item {
                Item::Node(id) => {
                    let mut node = self.canvas().node(id)?.clone();
                    node.color = color.clone();
                    Some(Change::UpdateNode { node })
                }
                Item::Edge(id) => {
                    let mut edge = self.canvas().edge(id)?.clone();
                    edge.color = color.clone();
                    Some(Change::UpdateEdge { edge })
                }
            })
            .collect::<Vec<_>>();
        self.submit(changes);
    }

    pub fn selection_text_size(&self) -> Option<f32> {
        let mut sizes = self.selection().iter().filter_map(|item| match item {
            Item::Node(id) => self
                .canvas()
                .node(id)
                .filter(|node| node.kind == model::TEXT)
                .map(Node::text_size),
            Item::Edge(id) => self.canvas().edge(id).map(|edge| edge.text_size()),
        });
        let first = sizes.next()?;
        sizes.all(|size| size == first).then_some(first)
    }

    pub fn selection_text_mode(&self) -> Option<TextMode> {
        let mut modes = self.selected_nodes().into_iter().filter_map(|id| {
            let node = self.canvas().node(id)?;
            (node.kind == model::TEXT).then_some(node.text_mode.unwrap_or(TextMode::Card))
        });
        let first = modes.next()?;
        modes.all(|mode| mode == first).then_some(first)
    }

    pub fn set_selection_text_size(&mut self, size: Option<f32>) {
        if size.is_some_and(|size| !size.is_finite() || size <= 0.) {
            return;
        }
        let changes = self
            .selection()
            .iter()
            .filter_map(|item| match item {
                Item::Node(id) => {
                    let mut node = self.canvas().node(id)?.clone();
                    if node.kind != model::TEXT {
                        return None;
                    }
                    node.font_size = size;
                    Some(Change::UpdateNode { node })
                }
                Item::Edge(id) => {
                    let mut edge = self.canvas().edge(id)?.clone();
                    edge.font_size = size;
                    Some(Change::UpdateEdge { edge })
                }
            })
            .collect::<Vec<_>>();
        self.submit(changes);
    }

    pub fn set_selection_text_mode(&mut self, mode: TextMode) {
        let changes = self
            .selected_nodes()
            .into_iter()
            .filter_map(|id| {
                let mut node = self.canvas().node(id)?.clone();
                if node.kind != model::TEXT {
                    return None;
                }
                node.text_mode = Some(mode);
                node.extra.remove(kind::MIN_HEIGHT);
                Some(Change::UpdateNode { node })
            })
            .collect::<Vec<_>>();
        self.submit(changes);
    }

    pub fn set_connection_ends(&mut self, from: End, to: End) {
        let Some(mut edge) = self
            .selected_edge()
            .and_then(|id| self.canvas().edge(id))
            .cloned()
        else {
            return;
        };
        edge.from_end = Some(from);
        edge.to_end = Some(to);
        self.submit([Change::UpdateEdge { edge }]);
    }
}
