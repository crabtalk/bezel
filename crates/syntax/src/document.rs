//! A source kept parsed across edits.
//!
//! [`crate::highlight`] parses the whole text on every call. A [`Document`]
//! holds the tree, takes each edit as it happens, and reparses from the old
//! tree, so a keystroke costs a reparse of what it touched.
//!
//! Spans come back the way [`crate::highlight`] paints them: of the patterns
//! that capture one node, the first in the query wins, and a node inside
//! another paints over it.
//!
//! [`Document::new`] answers `None` for a [`Grammar::Wasm`] language and for a
//! language with an injections query.

use crate::lang::{self, Grammar, Lang};
use std::{collections::HashSet, ops::Range};
use theme::HighlightKind;
use tree_sitter::{InputEdit, Parser, Point, Query, QueryCursor, StreamingIterator as _, Tree};

/// One text, its language, and its current tree.
pub struct Document {
    parser: Parser,
    query: Query,
    /// Per capture index, what it paints. `None` is a capture outside
    /// [`lang::NAMES`], which paints nothing but still claims its node.
    kinds: Vec<Option<HighlightKind>>,
    tree: Tree,
    source: String,
}

impl Document {
    /// Parse `source` as the language `tag` names.
    pub fn new(tag: &str, source: impl Into<String>) -> Option<Self> {
        Self::with_lang(lang::resolve(tag)?, source)
    }

    pub fn with_lang(lang: &'static Lang, source: impl Into<String>) -> Option<Self> {
        if !lang.injections.is_empty() {
            return None;
        }
        let language = match &lang.grammar {
            Grammar::Native(grammar) => tree_sitter::Language::from(*grammar),
            Grammar::Wasm(_) => return None,
        };
        let query = Query::new(&language, lang.query).ok()?;
        let kinds = query
            .capture_names()
            .iter()
            .map(|name| recognized(name).map(lang::kind_of))
            .collect();
        let mut parser = Parser::new();
        parser.set_language(&language).ok()?;
        let source = source.into();
        let tree = parser.parse(&source, None)?;
        Some(Self {
            parser,
            query,
            kinds,
            tree,
            source,
        })
    }

    /// The text as of the last edit.
    pub fn source(&self) -> &str {
        &self.source
    }

    /// `range` of the text held became the `new_end - range.start` bytes of
    /// `source` that start at `range.start`, and `source` is the whole text
    /// now. Reparses from the old tree and answers the byte ranges of `source`
    /// whose spans may have changed.
    pub fn edit(&mut self, range: Range<usize>, new_end: usize, source: &str) -> Vec<Range<usize>> {
        let edit = InputEdit {
            start_byte: range.start,
            old_end_byte: range.end,
            new_end_byte: new_end,
            start_position: point_at(&self.source, range.start),
            old_end_position: point_at(&self.source, range.end),
            new_end_position: point_at(source, new_end),
        };
        self.tree.edit(&edit);
        self.source.clear();
        self.source.push_str(source);
        let Some(tree) = self.parser.parse(&self.source, Some(&self.tree)) else {
            return std::iter::once(0..self.source.len()).collect();
        };
        let mut changed: Vec<Range<usize>> = self
            .tree
            .changed_ranges(&tree)
            .map(|range| range.start_byte..range.end_byte)
            .collect();
        // What was typed is changed whether or not the tree's shape moved.
        changed.push(range.start..new_end);
        self.tree = tree;
        merge(changed)
    }

    /// Spans over the whole text, in document order.
    pub fn spans(&self) -> Vec<(Range<usize>, HighlightKind)> {
        self.spans_in(0..self.source.len())
    }

    /// Spans over the part of the text `range` covers, clipped to it.
    pub fn spans_in(&self, range: Range<usize>) -> Vec<(Range<usize>, HighlightKind)> {
        let mut cursor = QueryCursor::new();
        cursor.set_byte_range(range.clone());
        let mut captures =
            cursor.captures(&self.query, self.tree.root_node(), self.source.as_bytes());
        // One per node: the first pattern to capture it.
        let mut seen = HashSet::new();
        let mut claimed: Vec<(Range<usize>, Option<HighlightKind>)> = Vec::new();
        while let Some((found, index)) = captures.next() {
            let capture = found.captures()[*index];
            if seen.insert(capture.node.id()) {
                claimed.push((
                    capture.node.byte_range(),
                    self.kinds[capture.index as usize],
                ));
            }
        }
        claimed.sort_by(|a, b| a.0.start.cmp(&b.0.start).then(b.0.end.cmp(&a.0.end)));
        flatten(
            claimed
                .into_iter()
                .filter_map(|(span, kind)| Some((span, kind?))),
            range,
        )
    }
}

/// Nested captures, outer first at a shared start, as flat spans where the
/// innermost wins, clipped to `within`.
fn flatten(
    captures: impl Iterator<Item = (Range<usize>, HighlightKind)>,
    within: Range<usize>,
) -> Vec<(Range<usize>, HighlightKind)> {
    let mut spans = Vec::new();
    let mut stack: Vec<(usize, HighlightKind)> = Vec::new();
    let mut at = within.start;
    let mut paint = |to: usize, stack: &[(usize, HighlightKind)], at: &mut usize| {
        let to = to.min(within.end);
        if to > *at {
            if let Some((_, kind)) = stack.last() {
                spans.push((*at..to, *kind));
            }
            *at = to;
        }
    };
    for (span, kind) in captures {
        while let Some(&(end, _)) = stack.last() {
            if end > span.start {
                break;
            }
            paint(end, &stack, &mut at);
            stack.pop();
        }
        paint(span.start, &stack, &mut at);
        stack.push((span.end, kind));
    }
    while let Some(&(end, _)) = stack.last() {
        paint(end, &stack, &mut at);
        stack.pop();
    }
    spans
}

/// The row and byte column of `offset` in `text`.
fn point_at(text: &str, offset: usize) -> Point {
    let before = &text.as_bytes()[..offset.min(text.len())];
    let row = before.iter().filter(|byte| **byte == b'\n').count();
    let column = before
        .iter()
        .rposition(|byte| *byte == b'\n')
        .map_or(before.len(), |newline| before.len() - newline - 1);
    Point { row, column }
}

/// Sorted and joined where they touch.
fn merge(mut ranges: Vec<Range<usize>>) -> Vec<Range<usize>> {
    ranges.sort_by_key(|range| range.start);
    let mut merged: Vec<Range<usize>> = Vec::new();
    for range in ranges {
        match merged.last_mut() {
            Some(last) if range.start <= last.end => last.end = last.end.max(range.end),
            _ => merged.push(range),
        }
    }
    merged
}

/// The entry of [`lang::NAMES`] a capture name paints as: the one with the
/// most dot-separated parts, all of them among the capture's. The rule
/// `tree_sitter_highlight::HighlightConfiguration::configure` applies.
fn recognized(capture: &str) -> Option<&'static str> {
    let parts: Vec<&str> = capture.split('.').collect();
    // The first of the longest, as `configure` breaks a tie.
    let mut best: Option<(&'static str, usize)> = None;
    for name in lang::NAMES {
        let len = name.split('.').count();
        if name.split('.').all(|part| parts.contains(&part))
            && best.is_none_or(|(_, longest)| len > longest)
        {
            best = Some((name, len));
        }
    }
    best.map(|(name, _)| name)
}
