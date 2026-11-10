use crate::{
    change::Change,
    edit::{self, CanvasEditor},
    kind::Capability,
    mindmap,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Arrange {
    Left,
    Center,
    Right,
    Top,
    Middle,
    Bottom,
    DistributeHorizontal,
    DistributeVertical,
    Row { gap: i64 },
    Column { gap: i64 },
    Grid { columns: usize, gap: i64 },
    JustifyHorizontal,
    JustifyVertical,
}

impl CanvasEditor {
    /// Arrange the selection as one undoable batch. Selected containers carry
    /// their contents; those contents are not arranged independently.
    pub fn arrange(&mut self, command: Arrange) {
        let selected = self.selected_nodes();
        let mut nodes: Vec<_> = selected
            .iter()
            .filter_map(|id| self.canvas().node(id))
            .filter(|node| self.node_can(&node.id, Capability::Draggable))
            .filter(|node| {
                !selected.iter().any(|other| {
                    *other != node.id && self.contents(&[(*other).to_owned()]).contains(&node.id)
                })
            })
            .cloned()
            .collect();
        if nodes.len() < 2 {
            return;
        }
        let Some((left, top, right, bottom)) = edit::extent(&nodes) else {
            return;
        };
        let horizontal = command == Arrange::DistributeHorizontal;
        if matches!(
            command,
            Arrange::DistributeHorizontal | Arrange::DistributeVertical
        ) {
            if nodes.len() < 3 {
                return;
            }
            nodes.sort_by_key(|node| if horizontal { node.x } else { node.y });
        }
        let mut targets = nodes.clone();
        match command {
            Arrange::Left
            | Arrange::Center
            | Arrange::Right
            | Arrange::Top
            | Arrange::Middle
            | Arrange::Bottom => {
                for node in &mut targets {
                    match command {
                        Arrange::Left => node.x = left,
                        Arrange::Center => node.x = left + (right - left - node.width) / 2,
                        Arrange::Right => node.x = right - node.width,
                        Arrange::Top => node.y = top,
                        Arrange::Middle => node.y = top + (bottom - top - node.height) / 2,
                        Arrange::Bottom => node.y = bottom - node.height,
                        _ => unreachable!(),
                    }
                }
            }
            Arrange::DistributeHorizontal | Arrange::DistributeVertical => {
                let length = if horizontal {
                    right - left
                } else {
                    bottom - top
                };
                let occupied: i64 = nodes
                    .iter()
                    .map(|node| if horizontal { node.width } else { node.height })
                    .sum();
                let gap = (length - occupied) as f64 / (nodes.len() - 1) as f64;
                let mut at = if horizontal { left } else { top } as f64;
                for node in &mut targets {
                    if horizontal {
                        node.x = at.round() as i64;
                        at += node.width as f64 + gap;
                    } else {
                        node.y = at.round() as i64;
                        at += node.height as f64 + gap;
                    }
                }
            }
            Arrange::Row { gap } | Arrange::Column { gap } | Arrange::Grid { gap, .. } => {
                let columns = match command {
                    Arrange::Row { .. } => nodes.len(),
                    Arrange::Column { .. } => 1,
                    Arrange::Grid { columns, .. } => columns.clamp(1, nodes.len()),
                    _ => unreachable!(),
                };
                let mut widths = vec![0; columns];
                let mut heights = vec![0; nodes.len().div_ceil(columns)];
                for (i, node) in nodes.iter().enumerate() {
                    widths[i % columns] = widths[i % columns].max(node.width);
                    heights[i / columns] = heights[i / columns].max(node.height);
                }
                for (i, node) in targets.iter_mut().enumerate() {
                    node.x = left
                        + widths[..i % columns].iter().sum::<i64>()
                        + (i % columns) as i64 * gap;
                    node.y = top
                        + heights[..i / columns].iter().sum::<i64>()
                        + (i / columns) as i64 * gap;
                }
            }
            Arrange::JustifyHorizontal | Arrange::JustifyVertical => {
                for node in &mut targets {
                    if !self.node_can(&node.id, Capability::Resizable) {
                        continue;
                    }
                    if command == Arrange::JustifyHorizontal {
                        node.x = left;
                        node.width = right - left;
                    } else {
                        node.y = top;
                        node.height = bottom - top;
                    }
                }
            }
        }
        let mut changes = Vec::new();
        for (before, after) in nodes.iter().zip(targets) {
            if (before.x, before.y) != (after.x, after.y) {
                let ids = self
                    .contents(std::slice::from_ref(&before.id))
                    .into_iter()
                    .filter(|id| self.node_can(id, Capability::Draggable))
                    .collect::<Vec<_>>();
                changes.extend(mindmap::carry(
                    self.canvas(),
                    &ids,
                    (after.x - before.x, after.y - before.y),
                    self.layout().pins,
                ));
            }
            if (before.width, before.height) != (after.width, after.height) {
                changes.push(Change::Resize {
                    id: before.id.clone(),
                    size: (after.width, after.height),
                });
            }
        }
        self.submit(changes);
    }
}
