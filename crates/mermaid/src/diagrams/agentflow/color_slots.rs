//! Two rules, as in Mermaid: a node takes the color of its kind, from a fixed
//! slot, so editing the diagram around it never recolors it; containers
//! count through the slots above the kinds, in the order they were declared,
//! so a frame does not take the color of a node inside it. Pele's
//! `diagrams/agentflow/colorSlots.ts`.

use indexmap::IndexMap;

use super::graph::GraphNode;

/// Each kind's slot, in the order the kinds were declared.
pub const KIND_SLOT: [(&str, usize); 7] = [
    ("tool", 0),
    ("task", 1),
    ("decision", 2),
    ("input", 3),
    ("refdoc", 4),
    ("connector", 5),
    ("action", 6),
];

pub const KIND_COUNT: usize = KIND_SLOT.len();

/// The slot of `kind`, when it is one.
pub fn kind_slot(kind: &str) -> Option<usize> {
    KIND_SLOT
        .iter()
        .find(|(name, _)| *name == kind)
        .map(|&(_, slot)| slot)
}

/// At least one, so that a palette shorter than the kinds still gives
/// containers a slot.
pub fn container_slot_count(palette_length: usize) -> usize {
    palette_length.saturating_sub(KIND_COUNT).max(1)
}

pub fn container_slot(n: usize, palette_length: usize) -> usize {
    let slot = KIND_COUNT + n % container_slot_count(palette_length);
    if palette_length > 0 {
        slot % palette_length
    } else {
        slot
    }
}

pub fn assign_color_slots(
    nodes: &mut [GraphNode],
    kind_of: impl Fn(&str) -> Option<&'static str>,
    container_order: &IndexMap<String, usize>,
    palette_length: usize,
) {
    let mut fallback = container_order.len();
    for node in nodes {
        // A collapsed container is drawn as a node but keeps its slot, so
        // collapsing one does not recolor the containers after it.
        if node.is_group || node.shape == "collapsedGroup" {
            let n = match container_order.get(&node.id) {
                Some(&n) => n,
                None => {
                    fallback += 1;
                    fallback - 1
                }
            };
            node.color_index = Some(container_slot(n, palette_length));
            continue;
        }
        if let Some(kind) = kind_of(&node.id)
            && kind_slot(kind).is_some()
        {
            node.kind = Some(kind);
        }
    }
}
