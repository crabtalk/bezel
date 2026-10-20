//! Mermaid parses use case diagrams with a Chevrotain parser, which decides
//! each choice by looking at most three tokens ahead and reports the first
//! token that fits nowhere. This parser makes the same decisions from the
//! same lookahead, so it accepts the same text and names the same token in
//! its errors. It builds the model directly, where Mermaid builds a syntax
//! tree and then visits it. Pele's `diagrams/usecase/parser.ts`.

use super::{
    db::{
        DraftBoundary, DraftClassDef, DraftClasses, DraftElement, DraftEndpoint, DraftJson,
        DraftLabel, DraftMetadata, DraftNote, DraftProperty, DraftRelationship, DraftStyle, Drafts,
        ElementKind, Named, PropertyValue,
    },
    json::parse_ordered_json,
    lexer::{Source, Token},
    tokens::T,
    types::{
        Arrow, Direction, EdgeOccurrence, GraphStatement, LabelType, NodeOccurrence,
        RelationshipType, Span, StatementKind, UseCaseShape, UsecaseModel,
    },
};
use crate::{
    error::Error,
    js,
    util::chars::{is_blank, is_word},
};

const FORBIDDEN: [&str; 5] = [
    "allowmixing",
    "newpage",
    "package",
    "rectangle",
    "skinparam",
];

/// What may follow an element's name in a statement, in Mermaid's order.
fn follows_name() -> impl Iterator<Item = T> {
    [T::StereotypeStart, T::ClassSeparator, T::Identifier]
        .into_iter()
        .chain(T::ARROWS)
        .chain([T::Newline, T::Eof])
}

impl T {
    fn starts_line(self) -> bool {
        matches!(self, T::Newline | T::Comment) || T::STATEMENTS.contains(&self) || self.is_name()
    }

    fn starts_relation(self) -> bool {
        self == T::Identifier || self.is_arrow()
    }

    fn starts_boundary_line(self) -> bool {
        matches!(self, T::Newline | T::Comment | T::Actor) || self.is_name()
    }

    /// The operators that can close a label opened by `--`.
    fn closes_label(self) -> bool {
        matches!(
            self,
            T::ForwardSolid | T::MarkerlessSolid | T::ForwardCircle | T::ForwardCross
        )
    }
}

#[derive(Clone, Default)]
struct Item {
    id: String,
    label: DraftLabel,
    span: Span,
    generated: bool,
    shape: Option<UseCaseShape>,
    metadata: Option<Vec<DraftProperty>>,
    stereotype: Option<String>,
    stereotype_span: Option<Span>,
    classes: Vec<String>,
    class_spans: Vec<Span>,
    /// Whether this mention says something about the element, and so
    /// declares it.
    explicit: bool,
    node_span: Span,
}

impl Item {
    fn endpoint(&self, declares: bool) -> DraftEndpoint {
        DraftEndpoint {
            named: self.named(),
            declares,
            classed: !self.classes.is_empty(),
        }
    }

    fn occurrence(&self, defines: bool) -> NodeOccurrence {
        NodeOccurrence {
            id: self.id.clone(),
            span: self.node_span,
            id_span: self.span,
            label_span: Some(self.label.span),
            defines: defines.then_some(true),
            stereotype_span: self.stereotype_span,
            metadata: self
                .metadata
                .as_ref()
                .map(|properties| properties.iter().map(DraftProperty::occurrence).collect()),
            class_spans: (!self.class_spans.is_empty()).then(|| self.class_spans.clone()),
        }
    }

    fn named(&self) -> Named {
        Named {
            id: self.id.clone(),
            label: self.label.clone(),
            span: self.span,
            generated: self.generated,
        }
    }
}

struct Tail {
    explicit_id: Option<String>,
    explicit_id_span: Option<Span>,
    kind: RelationshipType,
    arrow_type: Arrow,
    label: Option<DraftLabel>,
    minlen: usize,
    target: Item,
    span: Span,
}

/// An id made from a label: each character that is not a word character,
/// counted in UTF-16 units, becomes `_`.
fn generated_id(label: &str) -> String {
    let mut out = String::new();
    for c in label.chars() {
        if c.is_ascii_alphanumeric() || c == '_' {
            out.push(c);
        } else {
            for _ in 0..c.len_utf16() {
                out.push('_');
            }
        }
    }
    out
}

/// Chevrotain's list of the token sequences that start a top-level
/// statement.
fn statement_sequences() -> Vec<String> {
    let metadata: Vec<String> = T::NAMES
        .iter()
        .flat_map(|name| {
            T::METADATA_FIRST
                .iter()
                .map(move |next| format!("{}, METADATA_START, {}", name.name(), next.name()))
        })
        .collect();
    let entities = T::NAMES.iter().flat_map(|&name| {
        let open: &[T] = if name == T::Identifier {
            &[T::Lparen, T::Lbracket]
        } else {
            &[]
        };
        open.iter()
            .copied()
            .chain(follows_name())
            .map(move |next| format!("{}, {}", name.name(), next.name()))
    });
    T::STATEMENTS
        .iter()
        .map(|kind| kind.name().to_string())
        .chain(metadata.iter().cloned())
        .chain(entities)
        .chain(metadata.iter().cloned())
        .collect()
}

struct Parser<'a> {
    src: &'a Source,
    tokens: Vec<Token>,
    p: usize,
    /// Where the last token before the most recent line end stops, which is
    /// where its statement stops.
    last_end: usize,
    drafts: Drafts,
    parent: Option<DraftBoundary>,
    /// Drafts in `drafts.jsons`, and the token of their literal.
    pending_json: Vec<(usize, usize)>,
    edge_count: usize,
    note_count: usize,
    /// The index the top-level statement being read will have.
    top_index: usize,
}

impl Parser<'_> {
    fn t(&self, k: usize) -> T {
        self.tokens.get(k).map_or(T::Eof, |token| token.kind)
    }

    fn image(&self, k: usize) -> String {
        self.src.slice(self.tokens[k].start, self.tokens[k].end)
    }

    fn span(&self, k: usize) -> Span {
        self.tokens[k].span()
    }

    fn fail(&self, detail: &str) -> Error {
        let Token { start, end, .. } = self.tokens[self.p];
        self.src.place(
            Error::syntax(format!(
                "Error parsing usecase diagram: {detail} at {}",
                self.src.location_text(start, end)
            )),
            start,
        )
    }

    fn mismatch(&self, name: &str) -> Error {
        self.fail(&format!(
            "Expecting token of type --> {name} <-- but found --> '{}' <--",
            self.image(self.p)
        ))
    }

    fn expect(&mut self, kind: T) -> Result<usize, Error> {
        if self.t(self.p) != kind {
            return Err(self.mismatch(kind.name()));
        }
        self.p += 1;
        Ok(self.p - 1)
    }

    /// Chevrotain lists every token sequence that would have been accepted.
    fn no_alternative<S: AsRef<str>>(&self, sequences: impl IntoIterator<Item = S>) -> Error {
        let list: Vec<String> = sequences
            .into_iter()
            .enumerate()
            .map(|(i, sequence)| format!("  {}. [{}]", i + 1, sequence.as_ref()))
            .collect();
        self.fail(&format!(
            "Expecting: one of these possible Token sequences:\n{}\nbut found: '{}'",
            list.join("\n"),
            self.image(self.p)
        ))
    }

    fn line_end(&mut self) -> Result<(), Error> {
        self.last_end = self.tokens[self.p - 1].end;
        if self.t(self.p) == T::Newline {
            self.p += 1;
        } else if self.t(self.p) != T::Eof {
            return Err(self.no_alternative([T::Newline, T::Eof].map(T::name)));
        }
        Ok(())
    }

    /// The text of a name or string token, without its quotes.
    fn content(&self, k: usize) -> Span {
        let trim = match self.t(k) {
            T::MarkdownString => 2,
            T::PlainString => 1,
            _ => 0,
        };
        [self.tokens[k].start + trim, self.tokens[k].end - trim]
    }

    fn token_label(&self, k: usize) -> DraftLabel {
        let at = self.content(k);
        DraftLabel {
            text: self.src.slice(at[0], at[1]),
            kind: if self.t(k) == T::MarkdownString {
                LabelType::Markdown
            } else {
                LabelType::Text
            },
            span: at,
        }
    }

    /// An unquoted label runs to the next delimiter and is taken from the
    /// source as written.
    fn label(&mut self) -> Result<DraftLabel, Error> {
        let kind = self.t(self.p);
        if kind == T::PlainString || kind == T::MarkdownString {
            self.p += 1;
            return Ok(self.token_label(self.p - 1));
        }
        if !kind.is_label_text() {
            return Err(self.no_alternative([
                T::PlainString.name(),
                T::MarkdownString.name(),
                T::LABEL_TEXT,
            ]));
        }
        let first = self.p;
        while self.t(self.p).is_label_text() {
            self.p += 1;
        }
        let at: Span = [self.tokens[first].start, self.tokens[self.p - 1].end];
        Ok(DraftLabel {
            text: self.src.slice(at[0], at[1]),
            kind: LabelType::Text,
            span: at,
        })
    }

    fn property(&mut self) -> Result<DraftProperty, Error> {
        let key = self.p;
        if !self.t(self.p).is_key() {
            return Err(self.no_alternative([T::Identifier, T::PlainString].map(T::name)));
        }
        self.p += 1;
        self.expect(T::Colon)?;
        let value = self.p;
        let kind = self.t(self.p);
        if !kind.is_key() && kind != T::True && kind != T::False {
            return Err(self
                .no_alternative([T::Identifier, T::PlainString, T::True, T::False].map(T::name)));
        }
        self.p += 1;
        let key_span = self.content(key);
        let value_span = self.content(value);
        Ok(DraftProperty {
            key: self.src.slice(key_span[0], key_span[1]),
            value: match kind {
                T::True => PropertyValue::Bool(true),
                T::False => PropertyValue::Bool(false),
                _ => PropertyValue::Text(self.src.slice(value_span[0], value_span[1])),
            },
            span: [self.tokens[key].start, self.tokens[value].end],
            key_span,
            value_span,
        })
    }

    /// Whether another property follows. A comma or line breaks before the
    /// closing brace do not count, but Chevrotain looks only three tokens
    /// ahead, so a comma and two line breaks, or three line breaks, are taken
    /// for a separator and the brace after them is then an error.
    fn separator_ahead(&self) -> bool {
        let a = self.t(self.p);
        let b = self.t(self.p + 1);
        let c = self.t(self.p + 2);
        if a == T::Comma {
            return b.is_key() || (b == T::Newline && (c == T::Newline || c.is_key()));
        }
        if a != T::Newline {
            return false;
        }
        b == T::Comma
            || b.is_key()
            || (b == T::Newline && (c == T::Newline || c == T::Comma || c.is_key()))
    }

    fn metadata(&mut self) -> Result<Vec<DraftProperty>, Error> {
        self.expect(T::MetadataStart)?;
        let mut properties = Vec::new();
        while self.t(self.p) == T::Newline {
            self.p += 1;
        }
        if self.t(self.p).is_key() {
            properties.push(self.property()?);
            while self.separator_ahead() {
                if self.t(self.p) == T::Comma {
                    self.p += 1;
                } else {
                    while self.t(self.p) == T::Newline {
                        self.p += 1;
                    }
                    if self.t(self.p) == T::Comma {
                        self.p += 1;
                    }
                }
                while self.t(self.p) == T::Newline {
                    self.p += 1;
                }
                properties.push(self.property()?);
            }
            if self.t(self.p) == T::Comma {
                self.p += 1;
            }
            while self.t(self.p) == T::Newline {
                self.p += 1;
            }
        }
        self.expect(T::Rbrace)?;
        Ok(properties)
    }

    fn class_suffix(
        &mut self,
        classes: &mut Vec<String>,
        spans: &mut Vec<Span>,
    ) -> Result<(), Error> {
        loop {
            self.p += 1;
            let id = self.expect(T::Identifier)?;
            classes.push(self.image(id));
            spans.push(self.span(id));
            if self.t(self.p) != T::Comma {
                return Ok(());
            }
        }
    }

    /// An id with an optional label in parentheses or brackets, or a string
    /// that stands for both.
    fn name(&mut self, brackets: bool) -> Result<Item, Error> {
        let k = self.p;
        let kind = self.t(self.p);
        let mut item = Item {
            span: self.content(k),
            generated: kind != T::Identifier,
            ..Item::default()
        };
        if kind == T::Identifier {
            self.p += 1;
            item.id = self.image(k);
            let open = self.t(self.p);
            if open == T::Lparen || (brackets && open == T::Lbracket) {
                self.p += 1;
                item.label = self.label()?;
                self.expect(if open == T::Lparen {
                    T::Rparen
                } else {
                    T::Rbracket
                })?;
                item.shape = Some(if open == T::Lparen {
                    UseCaseShape::Ellipse
                } else {
                    UseCaseShape::Rect
                });
            } else {
                item.label = self.token_label(k);
            }
        } else if kind == T::PlainString || kind == T::MarkdownString {
            self.p += 1;
            item.label = self.token_label(k);
            item.id = generated_id(&item.label.text);
        } else {
            return Err(self.no_alternative(T::NAMES.map(T::name)));
        }
        Ok(item)
    }

    fn element(&mut self, actor: bool) -> Result<Item, Error> {
        let first = self.p;
        let mut item = self.name(!actor)?;
        if actor {
            item.shape = None;
        }
        if self.t(self.p) == T::MetadataStart {
            item.metadata = Some(self.metadata()?);
        }
        if self.t(self.p) == T::StereotypeStart {
            self.p += 1;
            let text = self.expect(T::StereotypeText)?;
            self.expect(T::StereotypeEnd)?;
            item.stereotype = Some(js::trim(&self.image(text)).to_string());
            item.stereotype_span = Some(self.span(text));
        }
        if self.t(self.p) == T::ClassSeparator {
            self.class_suffix(&mut item.classes, &mut item.class_spans)?;
        }
        item.explicit = item.shape.is_some()
            || item.metadata.is_some()
            || item.stereotype.as_deref().is_some_and(|s| !s.is_empty());
        item.node_span = [self.tokens[first].start, self.tokens[self.p - 1].end];
        Ok(item)
    }

    fn draft(&self, item: &Item, kind: ElementKind) -> DraftElement {
        DraftElement {
            named: item.named(),
            kind,
            parent: self.parent.clone(),
            shape: item.shape,
            metadata: item.metadata.clone(),
            stereotype: item.stereotype.clone(),
            classes: item.classes.clone(),
        }
    }

    /// Whether a label and then one of the operators that can close it come
    /// next.
    fn labeled_right(&self, closes: impl Fn(T) -> bool) -> bool {
        if !self.t(self.p).is_label_part() {
            return false;
        }
        let mut k = self.p + 1;
        loop {
            let kind = self.t(k);
            if kind == T::MarkerlessSolid || closes(kind) {
                return true;
            }
            if !kind.is_label_part() {
                return false;
            }
            k += 1;
        }
    }

    /// Each dash beyond the second asks for one more rank between the two
    /// ends.
    fn minimum_length(&self, k: usize) -> usize {
        let Token { kind, start, end } = self.tokens[k];
        let dashes = end - start - usize::from(kind != T::MarkerlessSolid);
        dashes.saturating_sub(1).max(1)
    }

    fn relation_tail(&mut self) -> Result<Tail, Error> {
        let first = self.p;
        let mut tail = Tail {
            explicit_id: None,
            explicit_id_span: None,
            kind: RelationshipType::Association,
            arrow_type: Arrow::Solid,
            label: None,
            minlen: 1,
            target: Item::default(),
            span: [0, 0],
        };
        if self.t(self.p) == T::Identifier && self.t(self.p + 1) == T::At {
            tail.explicit_id = Some(self.image(self.p));
            tail.explicit_id_span = Some(self.span(self.p));
            self.p += 2;
        }
        let k = self.p;
        let kind = self.t(self.p);
        self.p += 1;
        let width = self.tokens[k].end - self.tokens[k].start;
        match kind {
            T::DependencyArrow => {
                self.expect(T::Colon)?;
                let word = self.t(self.p);
                if word != T::Include && word != T::Extend {
                    return Err(self.no_alternative([T::Include, T::Extend].map(T::name)));
                }
                tail.kind = if word == T::Include {
                    RelationshipType::Include
                } else {
                    RelationshipType::Extend
                };
                tail.label = Some(DraftLabel {
                    text: tail.kind.name().to_string(),
                    kind: LabelType::Text,
                    span: self.span(self.p),
                });
                self.p += 1;
            }
            T::Generalization => tail.kind = RelationshipType::Generalization,
            T::ForwardSolid => tail.minlen = self.minimum_length(k),
            T::BackwardSolid => {
                tail.arrow_type = Arrow::Back;
                // Only the shortest operator can open a label:
                // `A <-- label -- B`.
                if width == 3 && self.labeled_right(|_| false) {
                    tail.label = Some(self.label()?);
                    let close = self.expect(T::MarkerlessSolid)?;
                    tail.minlen = self.minimum_length(close);
                } else {
                    tail.minlen = self.minimum_length(k);
                }
            }
            T::MarkerlessSolid => {
                tail.arrow_type = Arrow::Line;
                if width == 2 && self.labeled_right(T::closes_label) {
                    tail.label = Some(self.label()?);
                    let close = self.t(self.p);
                    if close == T::ForwardSolid {
                        tail.arrow_type = Arrow::Solid;
                    } else if close == T::ForwardCircle {
                        tail.arrow_type = Arrow::Circle;
                    } else if close == T::ForwardCross {
                        tail.arrow_type = Arrow::Cross;
                    } else if close != T::MarkerlessSolid {
                        return Err(self.no_alternative(
                            [
                                T::ForwardSolid,
                                T::MarkerlessSolid,
                                T::ForwardCircle,
                                T::ForwardCross,
                            ]
                            .map(T::name),
                        ));
                    }
                    if tail.arrow_type == Arrow::Solid || tail.arrow_type == Arrow::Line {
                        tail.minlen = self.minimum_length(self.p);
                    }
                    self.p += 1;
                } else {
                    tail.minlen = self.minimum_length(k);
                }
            }
            T::ForwardCircle => tail.arrow_type = Arrow::Circle,
            T::ForwardCross => tail.arrow_type = Arrow::Cross,
            T::BackwardCircle | T::BackwardCross => {
                tail.arrow_type = if kind == T::BackwardCircle {
                    Arrow::CircleReversed
                } else {
                    Arrow::CrossReversed
                };
                if self.labeled_right(|_| false) {
                    tail.label = Some(self.label()?);
                    self.expect(T::MarkerlessSolid)?;
                }
            }
            _ => {
                self.p -= 1;
                return Err(self.no_alternative(T::ARROWS.map(T::name)));
            }
        }
        tail.target = self.element(false)?;
        tail.span = [self.tokens[first].start, self.tokens[self.p - 1].end];
        Ok(tail)
    }

    fn relationship(
        &mut self,
        source: &Item,
        declaration: bool,
        tail: Tail,
        mut nodes: Vec<NodeOccurrence>,
    ) -> GraphStatement {
        let target = &tail.target;
        if target.explicit {
            let draft = self.draft(target, ElementKind::Usecase);
            self.drafts.elements.push(draft);
        }
        self.drafts.relationships.push(DraftRelationship {
            source: source.endpoint(declaration),
            target: target.endpoint(target.explicit),
            span: tail.span,
            id: tail.explicit_id.clone(),
            id_span: tail.explicit_id_span,
            kind: tail.kind,
            arrow_type: tail.arrow_type,
            label: tail.label.clone(),
            minlen: tail.minlen,
        });
        nodes.push(target.occurrence(target.explicit));
        let mut out = GraphStatement::with_nodes(StatementKind::Edge, nodes);
        let id = match &tail.explicit_id {
            Some(id) => id.clone(),
            None => {
                self.edge_count += 1;
                format!("edge-{}", self.edge_count - 1)
            }
        };
        out.edges = Some(vec![EdgeOccurrence {
            id,
            span: out.span,
            label_span: tail.label.as_ref().map(|label| label.span),
            id_span: tail.explicit_id_span,
            metadata: None,
        }]);
        out
    }

    /// `actor A, B` declares several actors; at the top level
    /// `actor A --> B` declares one and relates it.
    fn actors(&mut self, top: bool) -> Result<GraphStatement, Error> {
        self.p += 1;
        let mut items = vec![self.element(true)?];
        let mut tail: Option<Tail> = None;
        if self.t(self.p) == T::Comma {
            while self.t(self.p) == T::Comma {
                self.p += 1;
                items.push(self.element(true)?);
            }
        } else if top && self.t(self.p).starts_relation() {
            tail = Some(self.relation_tail()?);
        }
        self.line_end()?;
        for item in &items {
            let draft = self.draft(item, ElementKind::Actor);
            self.drafts.elements.push(draft);
        }
        let nodes: Vec<NodeOccurrence> = items.iter().map(|item| item.occurrence(true)).collect();
        Ok(match tail {
            Some(tail) => self.relationship(&items[0], true, tail, nodes),
            None => GraphStatement::with_nodes(StatementKind::Node, nodes),
        })
    }

    fn entity(&mut self, top: bool) -> Result<GraphStatement, Error> {
        let mut source = self.element(false)?;
        let tail = if top && self.t(self.p).starts_relation() {
            Some(self.relation_tail()?)
        } else {
            None
        };
        self.line_end()?;
        if tail.is_none() {
            source.explicit = true;
            if source.shape.is_none() {
                source.shape = Some(UseCaseShape::Ellipse);
            }
        }
        if source.explicit {
            let draft = self.draft(&source, ElementKind::Usecase);
            self.drafts.elements.push(draft);
        }
        let nodes = vec![source.occurrence(source.explicit)];
        Ok(match tail {
            Some(tail) => {
                let explicit = source.explicit;
                self.relationship(&source, explicit, tail, nodes)
            }
            None => GraphStatement::with_nodes(StatementKind::Node, nodes),
        })
    }

    fn boundary(&mut self) -> Result<GraphStatement, Error> {
        self.p += 1;
        let mut named = self.name(true)?;
        let inline = if self.t(self.p) == T::MetadataStart {
            Some(self.metadata()?)
        } else {
            None
        };
        if self.t(self.p) == T::ClassSeparator {
            self.class_suffix(&mut named.classes, &mut named.class_spans)?;
        }
        let group = DraftBoundary {
            named: named.named(),
            classes: named.classes.clone(),
        };
        self.line_end()?;
        self.drafts.boundaries.push(group.clone());
        self.parent = Some(group.clone());
        let mut children: Vec<GraphStatement> = Vec::new();
        while self.t(self.p).starts_boundary_line() {
            children.push(self.line(false)?);
        }
        self.parent = None;
        let end = self.expect(T::End)?;
        self.line_end()?;
        let mut out = GraphStatement::new(StatementKind::Group);
        out.group = Some(group.named.id.clone());
        out.id_span = Some(group.named.span);
        out.title_span = Some(group.named.label.span);
        out.end_span = Some(self.span(end));
        out.class_spans = Some(named.class_spans.clone());
        if !children.is_empty() {
            out.children = Some(children);
        }
        if let Some(inline) = inline {
            out.metadata = Some(inline.iter().map(DraftProperty::occurrence).collect());
            self.drafts.metadata.push(DraftMetadata {
                target: group.named.id.clone(),
                span: group.named.span,
                properties: inline,
                statement: None,
            });
        }
        Ok(out)
    }

    fn style(&mut self) -> Result<String, Error> {
        let first = self.p;
        let kind = self.t(self.p);
        if kind.is_word() || kind == T::CssIdentifier {
            self.p += 1;
        } else if kind == T::MarkerlessSolid {
            self.p += 1;
            if !self.t(self.p).is_word() {
                return Err(self.mismatch(T::WORD));
            }
            self.p += 1;
        } else {
            return Err(self.no_alternative([
                T::WORD,
                T::CssIdentifier.name(),
                T::MarkerlessSolid.name(),
            ]));
        }
        self.expect(T::Colon)?;
        if !self.t(self.p).is_style_part() {
            let parts: Vec<&str> = std::iter::once(T::WORD)
                .chain(T::STYLE_PARTS.map(T::name))
                .collect();
            return Err(self.fail(&format!(
                "Expecting: expecting at least one iteration which starts with one of these possible Token sequences::\n  <[{}]>\nbut found: '{}'",
                parts.join("] ,["),
                self.image(self.p)
            )));
        }
        while self.t(self.p).is_style_part() {
            self.p += 1;
        }
        // The value is taken from the source, so the spaces inside
        // `1px solid red` survive.
        Ok(self
            .src
            .slice(self.tokens[first].start, self.tokens[self.p - 1].end)
            .replace("\\,", ","))
    }

    fn styles(&mut self) -> Result<Vec<String>, Error> {
        let mut out = vec![self.style()?];
        while self.t(self.p) == T::Comma {
            self.p += 1;
            out.push(self.style()?);
        }
        Ok(out)
    }

    fn identifiers(&mut self) -> Result<Vec<usize>, Error> {
        let mut out = vec![self.expect(T::Identifier)?];
        while self.t(self.p) == T::Comma {
            self.p += 1;
            out.push(self.expect(T::Identifier)?);
        }
        Ok(out)
    }

    fn reference(&self, k: usize) -> NodeOccurrence {
        let at = self.content(k);
        NodeOccurrence {
            id: self.image(k),
            span: at,
            id_span: at,
            label_span: None,
            defines: None,
            stereotype_span: None,
            metadata: None,
            class_spans: None,
        }
    }

    fn top_statement(&mut self) -> Result<GraphStatement, Error> {
        let k = self.p;
        let kind = self.t(self.p);
        let mut out: GraphStatement;
        if kind == T::AccTitleLine || kind == T::AccDescrLine || kind == T::AccDescrBlock {
            self.p += 1;
            let text = self.image(k);
            let after = |c: char| text.find(c).map_or(0, |at| at + 1);
            if kind == T::AccDescrBlock {
                let open = after('{');
                let close = text.rfind('}').unwrap_or(0);
                self.drafts.acc_description =
                    js::trim(text.get(open..close.max(open)).unwrap_or("")).to_string();
            } else if kind == T::AccDescrLine {
                self.drafts.acc_description = js::trim(&text[after(':')..]).to_string();
            } else {
                self.drafts.acc_title = js::trim(&text[after(':')..]).to_string();
            }
            out = GraphStatement::new(if kind == T::AccTitleLine {
                StatementKind::AccTitle
            } else {
                StatementKind::AccDescr
            });
        } else if kind == T::Direction {
            self.p += 1;
            let value = self.t(self.p);
            if !(T::Td..=T::Rl).contains(&value) {
                return Err(self.no_alternative([T::Td, T::Tb, T::Bt, T::Lr, T::Rl].map(T::name)));
            }
            self.drafts.direction = Some(match value {
                T::Tb | T::Td => Direction::TB,
                T::Bt => Direction::BT,
                T::Lr => Direction::LR,
                _ => Direction::RL,
            });
            self.p += 1;
            out = GraphStatement::new(StatementKind::Direction);
        } else if kind == T::Actor {
            return self.actors(true);
        } else if kind == T::SystemBoundary {
            return self.boundary();
        } else if kind == T::Note {
            self.p += 1;
            self.expect(T::For)?;
            let target = self.expect(T::Identifier)?;
            let text = self.label()?;
            out = GraphStatement::new(StatementKind::Note);
            out.reference = Some(format!("note-{}", self.note_count));
            self.note_count += 1;
            out.ref_span = Some(text.span);
            out.nodes = Some(vec![self.reference(target)]);
            self.drafts.notes.push(DraftNote {
                target: self.image(target),
                span: self.span(target),
                label: text,
            });
        } else if kind == T::JsonDeclarationStart {
            self.p += 1;
            let literal = self.expect(T::JsonObjectLiteral)?;
            let mut classes: Vec<String> = Vec::new();
            let mut class_spans: Vec<Span> = Vec::new();
            if self.t(self.p) == T::ClassSeparator {
                self.class_suffix(&mut classes, &mut class_spans)?;
            }
            let mut from = self.tokens[k].start + 4;
            while self.src.matches(from, is_blank) {
                from += 1;
            }
            let mut to = from;
            while self.src.matches(to, is_word) {
                to += 1;
            }
            let json = DraftJson {
                id: self.src.slice(from, to),
                value: crate::config::Value::Object(crate::config::Map::new()),
                property_order: indexmap::IndexMap::new(),
                span: [from, to],
                classes,
            };
            out = GraphStatement::new(StatementKind::Json);
            out.nodes = Some(vec![NodeOccurrence {
                id: json.id.clone(),
                span: json.span,
                id_span: json.span,
                label_span: None,
                defines: Some(true),
                stereotype_span: None,
                metadata: None,
                class_spans: Some(class_spans.clone()),
            }]);
            out.class_spans = Some(class_spans);
            self.drafts.jsons.push(json);
            self.pending_json
                .push((self.drafts.jsons.len() - 1, literal));
        } else if kind == T::ClassDef {
            self.p += 1;
            let ids = self.identifiers()?;
            let styles = self.styles()?;
            self.drafts.class_defs.push(DraftClassDef {
                ids: ids.iter().map(|&id| self.image(id)).collect(),
                styles,
            });
            out = GraphStatement::new(StatementKind::ClassDef);
            out.reference = Some(self.image(ids[0]));
            out.ref_span = Some(self.span(ids[0]));
        } else if kind == T::Class {
            self.p += 1;
            let targets = self.identifiers()?;
            let classes = self.identifiers()?;
            self.drafts.classes.push(DraftClasses {
                targets: targets
                    .iter()
                    .map(|&id| (self.image(id), self.span(id)))
                    .collect(),
                classes: classes.iter().map(|&id| self.image(id)).collect(),
            });
            out = GraphStatement::new(StatementKind::ClassAssign);
            out.reference = Some(self.image(classes[0]));
            out.ref_span = Some(self.span(classes[0]));
            out.nodes = Some(targets.iter().map(|&id| self.reference(id)).collect());
        } else if kind == T::Style {
            self.p += 1;
            let target = self.expect(T::Identifier)?;
            let styles = self.styles()?;
            self.drafts.styles.push(DraftStyle {
                target: self.image(target),
                span: self.span(target),
                styles,
            });
            out = GraphStatement::new(StatementKind::Style);
            out.nodes = Some(vec![self.reference(target)]);
        } else {
            let next = self.t(self.p + 1);
            if next == T::MetadataStart {
                if !T::METADATA_FIRST.contains(&self.t(self.p + 2)) {
                    return Err(self.no_alternative(statement_sequences()));
                }
                self.p += 1;
                let mut target = self.reference(k);
                if kind != T::Identifier {
                    target.id = generated_id(&self.src.slice(target.span[0], target.span[1]));
                }
                let assigned = self.metadata()?;
                out = GraphStatement::new(StatementKind::Metadata);
                out.metadata = Some(assigned.iter().map(DraftProperty::occurrence).collect());
                self.drafts.metadata.push(DraftMetadata {
                    target: target.id.clone(),
                    span: target.span,
                    properties: assigned,
                    statement: Some(self.top_index),
                });
                out.nodes = Some(vec![target]);
            } else {
                let follows = follows_name().any(|kind| kind == next)
                    || (kind == T::Identifier && (next == T::Lparen || next == T::Lbracket));
                // Mermaid turns away the PlantUML statements people are most
                // likely to try.
                if !follows
                    || (kind == T::Identifier
                        && FORBIDDEN.contains(&self.image(k).to_lowercase().as_str()))
                {
                    return Err(self.no_alternative(statement_sequences()));
                }
                return self.entity(true);
            }
        }
        self.line_end()?;
        Ok(out)
    }

    fn line(&mut self, top: bool) -> Result<GraphStatement, Error> {
        let k = self.p;
        let kind = self.t(self.p);
        if kind == T::Newline {
            self.p += 1;
            let mut out = GraphStatement::new(StatementKind::Blank);
            out.span = self.span(k);
            return Ok(out);
        }
        if kind == T::Comment {
            self.p += 1;
            self.line_end()?;
            let mut out = GraphStatement::new(StatementKind::Comment);
            out.span = self.span(k);
            return Ok(out);
        }
        let mut out = if top {
            self.top_statement()?
        } else if kind == T::Actor {
            self.actors(false)?
        } else {
            self.entity(false)?
        };
        out.span = [self.tokens[k].start, self.last_end];
        if let Some(edges) = &mut out.edges {
            for edge in edges {
                edge.span = out.span;
            }
        }
        Ok(out)
    }
}

pub fn parse_usecase(source: &str, title: Option<&str>) -> Result<UsecaseModel, Error> {
    let src = Source::new(source);
    let mut p = Parser {
        src: &src,
        tokens: src.tokenize()?,
        p: 0,
        last_end: 0,
        drafts: Drafts::default(),
        parent: None,
        pending_json: Vec::new(),
        edge_count: 0,
        note_count: 0,
        top_index: 0,
    };
    let header = p.expect(T::Usecase)?;
    p.line_end()?;
    let mut statements: Vec<GraphStatement> = Vec::new();
    while p.t(p.p).starts_line() {
        p.top_index = statements.len();
        statements.push(p.line(true)?);
    }
    if p.t(p.p) != T::Eof {
        return Err(p.fail(&format!(
            "Redundant input, expecting EOF but found: {}",
            p.image(p.p)
        )));
    }
    // The whole text has to be well formed before the JSON in it is looked
    // at, as in Mermaid.
    for &(json, literal) in &p.pending_json {
        let start = p.tokens[literal].start;
        let parsed = parse_ordered_json(&p.image(literal), || src.line_column(start))?;
        p.drafts.jsons[json].value = parsed.value;
        p.drafts.jsons[json].property_order = parsed.property_order;
    }
    let header_span = p.span(header);
    let drafts = std::mem::take(&mut p.drafts);
    drafts.resolve(&src, header_span, statements, title)
}
