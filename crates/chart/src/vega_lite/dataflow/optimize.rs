//! Vega-Lite's dataflow optimizer, as `optimizeDataflow` runs it: passes
//! that take out nodes no unit reads, and then passes that also merge bins,
//! time units, aggregates with the same groups, identical nodes and the
//! outputs at forks, each round until a pass changes nothing, five passes
//! at most. Children move as Vega-Lite's own array splicing moves them.

use crate::vega_lite::dataflow::{Node, Tree};
use gpui::SharedString;
use std::cmp::Reverse;

/// Passes per round at most.
const MAX_RUNS: usize = 5;

impl Tree {
    pub(crate) fn optimize(&mut self) {
        for first in [true, false] {
            for _ in 0..MAX_RUNS {
                if !self.pass(first) {
                    break;
                }
            }
        }
    }

    /// One pass over every root: whether it changed the tree.
    fn pass(&mut self, first: bool) -> bool {
        let mut modified = false;
        self.drop_empty_roots();
        for root in self.roots.clone() {
            modified |= self.remove_unused(root);
        }
        self.drop_empty_roots();
        if !first {
            for root in self.roots.clone() {
                modified |= self.merge_bins(root);
            }
            for root in self.roots.clone() {
                modified |= self.remove_duplicate_time_units(root, &[]);
            }
            for root in self.roots.clone() {
                modified |= self.merge_aggregates(root);
            }
            for root in self.roots.clone() {
                modified |= self.merge_time_units(root);
            }
            for root in self.roots.clone() {
                modified |= self.merge_identical(root);
            }
            for root in self.roots.clone() {
                modified |= self.merge_outputs(root);
            }
        }
        modified
    }

    fn drop_empty_roots(&mut self) {
        let slots = &self.slots;
        self.roots.retain(|&root| !slots[root].children.is_empty());
    }

    /// The nodes under `root`, deepest first, those as deep in the order
    /// met: the order Vega-Lite's bottom-up passes visit them.
    fn bottom_up(&self, root: usize) -> Vec<usize> {
        let mut depths = Vec::new();
        let mut stack = vec![(root, 0)];
        while let Some((node, depth)) = stack.pop() {
            depths.push((node, depth));
            for &child in self.slots[node].children.iter().rev() {
                stack.push((child, depth + 1));
            }
        }
        depths.sort_by_key(|&(_, depth)| Reverse(depth));
        depths.into_iter().map(|(node, _)| node).collect()
    }

    /// `RemoveUnusedSubtrees`: takes out every leaf but an output or a
    /// source, until none is left.
    fn remove_unused(&mut self, root: usize) -> bool {
        let mut modified = false;
        for node in self.bottom_up(root) {
            let slot = &self.slots[node];
            if matches!(slot.node, Node::Output | Node::Source(_)) || !slot.children.is_empty() {
                continue;
            }
            modified = true;
            if let Some(parent) = slot.parent {
                self.retap(node, parent);
            }
            self.remove(node);
        }
        modified
    }

    /// `MergeBins`: a fork's bins merge into one, which moves above the fork
    /// where the fork neither starts nor filters the data nor writes a field
    /// the bins read.
    fn merge_bins(&mut self, root: usize) -> bool {
        let mut modified = false;
        for node in self.bottom_up(root) {
            let moves = !matches!(self.slots[node].node, Node::Source(_) | Node::Filter(_));
            let produced = self.slots[node].node.produces();
            let mut promotable = Vec::new();
            let mut remaining = Vec::new();
            for &child in &self.slots[node].children {
                let child_node = &self.slots[child].node;
                if !matches!(child_node, Node::Bin(_)) {
                    continue;
                }
                let reads: Vec<SharedString> = child_node.binned();
                match moves && !reads.iter().any(|field| produced.contains(field)) {
                    true => promotable.push(child),
                    false => remaining.push(child),
                }
            }
            if let Some(promoted) = promotable.pop() {
                for bin in promotable {
                    self.merge_into(promoted, bin);
                }
                modified = true;
                match self.slots[node].node {
                    Node::Bin(_) => self.merge_into(node, promoted),
                    _ => {
                        self.retap(promoted, node);
                        self.swap_with_parent(promoted);
                    }
                }
            }
            if remaining.len() > 1 {
                let kept = remaining.pop().expect("two or more");
                for bin in remaining {
                    self.merge_into(kept, bin);
                }
                modified = true;
            }
        }
        modified
    }

    /// `RemoveDuplicateTimeUnits`: takes out a time unit node that writes a
    /// field one above it wrote.
    fn remove_duplicate_time_units(&mut self, node: usize, above: &[SharedString]) -> bool {
        let mut modified = false;
        let mut fields = above.to_vec();
        if let Node::TimeUnit(_) = self.slots[node].node {
            let produced = self.slots[node].node.produces();
            if produced.iter().any(|field| above.contains(field)) {
                modified = true;
                if let Some(parent) = self.slots[node].parent {
                    self.retap(node, parent);
                }
                self.remove(node);
            }
            fields.extend(produced);
        }
        for child in self.slots[node].children.clone() {
            modified |= self.remove_duplicate_time_units(child, &fields);
        }
        modified
    }

    /// `MergeAggregates`: a fork's aggregates over the same groups merge into
    /// the last of them.
    fn merge_aggregates(&mut self, root: usize) -> bool {
        let mut modified = false;
        for node in self.bottom_up(root) {
            let mut groups: Vec<(Vec<SharedString>, Vec<usize>)> = Vec::new();
            for &child in &self.slots[node].children {
                let Node::Aggregate(op) = &self.slots[child].node else {
                    continue;
                };
                match groups
                    .iter_mut()
                    .find(|(groupby, _)| *groupby == op.groupby)
                {
                    Some((_, members)) => members.push(child),
                    None => groups.push((op.groupby.clone(), vec![child])),
                }
            }
            for (_, mut members) in groups {
                if members.len() < 2 {
                    continue;
                }
                let merged = members.pop().expect("two or more");
                for agg in members {
                    let other = self.slots[agg].node.clone();
                    self.slots[merged].node.absorb(&other);
                    self.remove_child(node, agg);
                    self.set_parent(agg, merged);
                    self.retap(agg, merged);
                    self.remove(agg);
                    modified = true;
                }
            }
        }
        modified
    }

    /// `MergeTimeUnits`: a fork's time units merge into the last of them.
    fn merge_time_units(&mut self, root: usize) -> bool {
        let mut modified = false;
        for node in self.bottom_up(root) {
            let mut units: Vec<usize> = self.slots[node]
                .children
                .iter()
                .copied()
                .filter(|&child| matches!(self.slots[child].node, Node::TimeUnit(_)))
                .collect();
            let Some(combination) = units.pop() else {
                continue;
            };
            for unit in units {
                modified = true;
                self.merge_into(combination, unit);
            }
        }
        modified
    }

    /// `MergeIdenticalNodes`: a fork's children alike merge into the first of
    /// them, from the roots down.
    fn merge_identical(&mut self, node: usize) -> bool {
        let mut modified = false;
        let mut buckets: Vec<Vec<usize>> = Vec::new();
        for &child in &self.slots[node].children {
            match buckets
                .iter_mut()
                .find(|bucket| self.slots[bucket[0]].node.same(&self.slots[child].node))
            {
                Some(bucket) => bucket.push(child),
                None => buckets.push(vec![child]),
            }
        }
        for bucket in buckets {
            let Some((&merged, rest)) = bucket.split_first() else {
                continue;
            };
            for &other in rest {
                modified = true;
                self.remove_child(node, other);
                self.set_parent(other, merged);
                self.retap(other, merged);
                self.remove(other);
            }
        }
        let mut i = 0;
        while let Some(&child) = self.slots[node].children.get(i) {
            modified |= self.merge_identical(child);
            i += 1;
        }
        modified
    }

    /// `MergeOutputs`: a fork's chains of outputs move above the fork, the
    /// fork's other children under them.
    fn merge_outputs(&mut self, root: usize) -> bool {
        let mut modified = false;
        for node in self.bottom_up(root) {
            let children = self.slots[node].children.clone();
            let is_output = |tree: &Self, n: usize| matches!(tree.slots[n].node, Node::Output);
            if !children.iter().any(|&child| is_output(self, child)) || children.len() <= 1 {
                continue;
            }
            let mut others: Vec<usize> = Vec::new();
            let mut main: Option<usize> = None;
            for child in children {
                if !is_output(self, child) {
                    others.push(child);
                    continue;
                }
                let mut last = child;
                while let [only] = self.slots[last].children[..]
                    && is_output(self, only)
                {
                    last = only;
                }
                others.extend(self.slots[last].children.clone());
                match main {
                    Some(main) => {
                        self.remove_child(node, child);
                        let parent = self.slots[main].parent.expect("an output has a parent");
                        self.set_parent(child, parent);
                        self.remove_child(parent, main);
                        self.set_parent(main, last);
                        modified = true;
                    }
                    None => main = Some(last),
                }
            }
            if let Some(main) = main
                && !others.is_empty()
            {
                modified = true;
                for child in others {
                    if let Some(parent) = self.slots[child].parent {
                        self.remove_child(parent, child);
                    }
                    self.set_parent(child, main);
                }
            }
        }
        modified
    }

    /// A bin's or time unit's `merge`: `into` takes `other`'s formulas and
    /// children, but those its splicing skips, which go to `other`'s parent
    /// with `other` taken out.
    fn merge_into(&mut self, into: usize, other: usize) {
        let node = self.slots[other].node.clone();
        self.slots[into].node.absorb(&node);
        let mut i = 0;
        while let Some(&child) = self.slots[other].children.get(i) {
            self.remove_child(other, child);
            self.set_parent(child, into);
            i += 1;
        }
        self.retap(other, into);
        self.remove(other);
    }

    /// Moves every tap on `from` to `to`.
    fn retap(&mut self, from: usize, to: usize) {
        for tap in &mut self.taps {
            if *tap == from {
                *tap = to;
            }
        }
    }

    /// `parent` as `node`'s, `node` its last child.
    pub(crate) fn set_parent(&mut self, node: usize, parent: usize) {
        self.slots[node].parent = Some(parent);
        self.add_child(parent, node, None);
    }

    /// `child` among `node`'s children at `at`, JavaScript's `splice` index,
    /// or last; once at most.
    fn add_child(&mut self, node: usize, child: usize, at: Option<isize>) {
        let children = &mut self.slots[node].children;
        if children.contains(&child) {
            return;
        }
        match at {
            Some(at) => {
                let len = children.len() as isize;
                let at = match at < 0 {
                    true => (len + at).max(0),
                    false => at.min(len),
                };
                children.insert(at as usize, child);
            }
            None => children.push(child),
        }
    }

    /// Takes `child` out of `node`'s children, and answers where it was; the
    /// last child, and -1, where `child` is not among them.
    fn remove_child(&mut self, node: usize, child: usize) -> isize {
        let children = &mut self.slots[node].children;
        match children.iter().position(|&c| c == child) {
            Some(at) => {
                children.remove(at);
                at as isize
            }
            None => {
                children.pop();
                -1
            }
        }
    }

    /// Takes `node` out from under its parent, its children in its place.
    fn remove(&mut self, node: usize) {
        let Some(parent) = self.slots[node].parent else {
            return;
        };
        let at = self.remove_child(parent, node);
        for (at, child) in (at..).zip(self.slots[node].children.clone()) {
            self.slots[child].parent = Some(parent);
            self.add_child(parent, child, Some(at));
        }
    }

    /// Puts `node` in its parent's place, its parent under it and its
    /// children under its parent.
    fn swap_with_parent(&mut self, node: usize) {
        let parent = self.slots[node].parent.expect("a parent");
        let grand = self.slots[parent].parent.expect("a grandparent");
        for child in self.slots[node].children.clone() {
            self.set_parent(child, parent);
        }
        self.slots[node].children.clear();
        self.remove_child(parent, node);
        let at = self.remove_child(grand, parent);
        self.slots[node].parent = Some(grand);
        self.add_child(grand, node, Some(at));
        self.set_parent(parent, node);
    }
}
