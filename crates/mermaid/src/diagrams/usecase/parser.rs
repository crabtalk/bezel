//! Mermaid parses use case diagrams with a Chevrotain parser, which decides
//! each choice by looking at most three tokens ahead and reports the first
//! token that fits nowhere. This parser makes the same decisions from the
//! same lookahead, so it accepts the same text and names the same token in
//! its errors. It builds the model directly, where Mermaid builds a syntax
//! tree and then visits it. Pele's `diagrams/usecase/parser.ts`.

use std::sync::LazyLock;

use super::{
    db::{
        DraftBoundary, DraftClassDef, DraftClasses, DraftElement, DraftEndpoint, DraftJson,
        DraftLabel, DraftMetadata, DraftNote, DraftProperty, DraftRelationship, DraftStyle, Drafts,
        ElementKind, Named, PropertyValue, resolve_usecase,
    },
    json::parse_ordered_json,
    lexer::{Source, Tokens},
    tokens::{T, TOKEN_NAMES},
    types::{
        Arrow, Direction, EdgeOccurrence, GraphStatement, LabelType, NodeOccurrence,
        RelationshipType, Span, StatementKind, UseCaseShape, UsecaseModel,
    },
};
use crate::{detect::Kind, error::Error, js};

const LINE: u8 = 1;
const RELATION: u8 = 2;
const STYLE_PART: u8 = 4;
const FOLLOWS_NAME: u8 = 8;
const BOUNDARY_LINE: u8 = 16;
const CLOSES_LABEL: u8 = 32;
const METADATA_FIRST: u8 = 64;
const ARROWS: &str = "DEPENDENCY_ARROW|GENERALIZATION|FORWARD_SOLID|BACKWARD_SOLID|MARKERLESS_SOLID|FORWARD_CIRCLE|BACKWARD_CIRCLE|FORWARD_CROSS|BACKWARD_CROSS";
const NAMES: &str = "IDENTIFIER|PLAIN_STRING|MARKDOWN_STRING";
const LABELS: &str = "PLAIN_STRING|MARKDOWN_STRING|LABEL_TEXT";
const STATEMENTS: &str = "ACC_TITLE_LINE|ACC_DESCR_LINE|ACC_DESCR_BLOCK|DIRECTION|ACTOR|SYSTEM_BOUNDARY|NOTE|JSON_DECLARATION_START|CLASS_DEF|CLASS|STYLE";
const STYLE_PARTS: &str = "WORD|PLAIN_STRING|NUMBER|HASH_COLOR|CSS_IDENTIFIER|CSS_ESCAPED_COMMA|DASH|DOT|PERCENT|CSS_PUNCTUATION|COLON|LPAREN|RPAREN|LBRACKET|RBRACKET|LBRACE|RBRACE|AT|MARKERLESS_SOLID";
const FORBIDDEN: [&str; 5] = [
    "allowmixing",
    "newpage",
    "package",
    "rectangle",
    "skinparam",
];

static FLAGS: LazyLock<[u8; T::RPAREN + 1]> = LazyLock::new(|| {
    let mut flags = [0u8; T::RPAREN + 1];
    let mut flag = |names: &str, bits: u8| {
        for name in names.split('|') {
            // WORD is a category: every keyword and IDENTIFIER.
            if name == "WORD" {
                for flag in &mut flags[T::USECASE..=T::IDENTIFIER] {
                    *flag |= bits;
                }
            } else if let Some(t) = T::named(name) {
                flags[t] |= bits;
            }
        }
    };
    flag(&format!("NEWLINE|COMMENT|{STATEMENTS}|{NAMES}"), LINE);
    flag(&format!("IDENTIFIER|{ARROWS}"), RELATION);
    flag(STYLE_PARTS, STYLE_PART);
    flag(
        &format!("STEREOTYPE_START|CLASS_SEPARATOR|IDENTIFIER|{ARROWS}|NEWLINE|EOF"),
        FOLLOWS_NAME,
    );
    flag(&format!("NEWLINE|COMMENT|ACTOR|{NAMES}"), BOUNDARY_LINE);
    flag(
        "FORWARD_SOLID|MARKERLESS_SOLID|FORWARD_CIRCLE|FORWARD_CROSS",
        CLOSES_LABEL,
    );
    flag("NEWLINE|IDENTIFIER|PLAIN_STRING|RBRACE", METADATA_FIRST);
    flags
});

#[derive(Clone)]
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
    arrow_type: usize,
    label: Option<DraftLabel>,
    minlen: usize,
    target: Item,
    span: Span,
}

fn is_word(kind: usize) -> bool {
    (T::USECASE..=T::IDENTIFIER).contains(&kind)
}

fn is_label_text(kind: usize) -> bool {
    (T::USECASE..=T::LABEL_SYMBOL).contains(&kind)
}

fn is_key(kind: usize) -> bool {
    kind == T::IDENTIFIER || kind == T::PLAIN_STRING
}

fn flags(kind: usize) -> u8 {
    FLAGS.get(kind).copied().unwrap_or(0)
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

fn statement_sequences() -> String {
    let names: Vec<&str> = NAMES.split('|').collect();
    let metadata: Vec<String> = names
        .iter()
        .flat_map(|name| {
            ["NEWLINE", "IDENTIFIER", "PLAIN_STRING", "RBRACE"]
                .iter()
                .map(move |next| format!("{name}, METADATA_START, {next}"))
        })
        .collect();
    let follows_text = format!("STEREOTYPE_START|CLASS_SEPARATOR|IDENTIFIER|{ARROWS}|NEWLINE|EOF");
    let follows: Vec<&str> = follows_text.split('|').collect();
    let mut entities: Vec<String> = Vec::new();
    for (i, name) in names.iter().enumerate() {
        let mut nexts: Vec<&str> = Vec::new();
        if i == 0 {
            nexts.extend(["LPAREN", "LBRACKET"]);
        }
        nexts.extend(follows.iter());
        for next in nexts {
            entities.push(format!("{name}, {next}"));
        }
    }
    let mut all: Vec<String> = vec![STATEMENTS.to_string()];
    all.extend(metadata.iter().cloned());
    all.extend(entities);
    all.extend(metadata);
    all.join("|")
}

struct Parser<'a> {
    src: &'a Source,
    types: Vec<usize>,
    starts: Vec<usize>,
    ends: Vec<usize>,
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
    fn t(&self, k: usize) -> usize {
        self.types.get(k).copied().unwrap_or(T::EOF)
    }

    fn image(&self, k: usize) -> String {
        self.src.slice(self.starts[k], self.ends[k])
    }

    fn span(&self, k: usize) -> Span {
        [self.starts[k], self.ends[k]]
    }

    fn fail(&self, detail: &str) -> Error {
        let at = self.starts[self.p];
        let (line, column) = self.src.line_column(at);
        let mut error = Error::syntax(format!(
            "Error parsing usecase diagram: {detail} at line {line}, column {column} [{at},{})",
            self.ends[self.p]
        ))
        .of(Kind::Usecase);
        error.line = line;
        error.column = column;
        error
    }

    fn mismatch(&self, name: &str) -> Error {
        self.fail(&format!(
            "Expecting token of type --> {name} <-- but found --> '{}' <--",
            self.image(self.p)
        ))
    }

    fn expect(&mut self, kind: usize) -> Result<usize, Error> {
        if self.t(self.p) != kind {
            return Err(self.mismatch(TOKEN_NAMES[kind]));
        }
        self.p += 1;
        Ok(self.p - 1)
    }

    /// Chevrotain lists every token sequence that would have been accepted.
    fn no_alternative(&self, expected: &str) -> Error {
        let list: Vec<String> = expected
            .split('|')
            .enumerate()
            .map(|(i, sequence)| format!("  {}. [{sequence}]", i + 1))
            .collect();
        self.fail(&format!(
            "Expecting: one of these possible Token sequences:\n{}\nbut found: '{}'",
            list.join("\n"),
            self.image(self.p)
        ))
    }

    fn line_end(&mut self) -> Result<(), Error> {
        self.last_end = self.ends[self.p - 1];
        if self.t(self.p) == T::NEWLINE {
            self.p += 1;
        } else if self.t(self.p) != T::EOF {
            return Err(self.no_alternative("NEWLINE|EOF"));
        }
        Ok(())
    }

    /// The text of a name or string token, without its quotes.
    fn content(&self, k: usize) -> Span {
        let trim = match self.t(k) {
            T::MARKDOWN_STRING => 2,
            T::PLAIN_STRING => 1,
            _ => 0,
        };
        [self.starts[k] + trim, self.ends[k] - trim]
    }

    fn token_label(&self, k: usize) -> DraftLabel {
        let at = self.content(k);
        DraftLabel {
            text: self.src.slice(at[0], at[1]),
            kind: if self.t(k) == T::MARKDOWN_STRING {
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
        if kind == T::PLAIN_STRING || kind == T::MARKDOWN_STRING {
            self.p += 1;
            return Ok(self.token_label(self.p - 1));
        }
        if !is_label_text(kind) {
            return Err(self.no_alternative(LABELS));
        }
        let first = self.p;
        while is_label_text(self.t(self.p)) {
            self.p += 1;
        }
        let at: Span = [self.starts[first], self.ends[self.p - 1]];
        Ok(DraftLabel {
            text: self.src.slice(at[0], at[1]),
            kind: LabelType::Text,
            span: at,
        })
    }

    fn property(&mut self) -> Result<DraftProperty, Error> {
        let key = self.p;
        if !is_key(self.t(self.p)) {
            return Err(self.no_alternative("IDENTIFIER|PLAIN_STRING"));
        }
        self.p += 1;
        self.expect(T::COLON)?;
        let value = self.p;
        let kind = self.t(self.p);
        if !is_key(kind) && kind != T::TRUE && kind != T::FALSE {
            return Err(self.no_alternative("IDENTIFIER|PLAIN_STRING|TRUE|FALSE"));
        }
        self.p += 1;
        let key_span = self.content(key);
        let value_span = self.content(value);
        Ok(DraftProperty {
            key: self.src.slice(key_span[0], key_span[1]),
            value: match kind {
                T::TRUE => PropertyValue::Bool(true),
                T::FALSE => PropertyValue::Bool(false),
                _ => PropertyValue::Text(self.src.slice(value_span[0], value_span[1])),
            },
            span: [self.starts[key], self.ends[value]],
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
        if a == T::COMMA {
            return is_key(b) || (b == T::NEWLINE && (c == T::NEWLINE || is_key(c)));
        }
        if a != T::NEWLINE {
            return false;
        }
        b == T::COMMA
            || is_key(b)
            || (b == T::NEWLINE && (c == T::NEWLINE || c == T::COMMA || is_key(c)))
    }

    fn metadata(&mut self) -> Result<Vec<DraftProperty>, Error> {
        self.expect(T::METADATA_START)?;
        let mut properties = Vec::new();
        while self.t(self.p) == T::NEWLINE {
            self.p += 1;
        }
        if is_key(self.t(self.p)) {
            properties.push(self.property()?);
            while self.separator_ahead() {
                if self.t(self.p) == T::COMMA {
                    self.p += 1;
                } else {
                    while self.t(self.p) == T::NEWLINE {
                        self.p += 1;
                    }
                    if self.t(self.p) == T::COMMA {
                        self.p += 1;
                    }
                }
                while self.t(self.p) == T::NEWLINE {
                    self.p += 1;
                }
                properties.push(self.property()?);
            }
            if self.t(self.p) == T::COMMA {
                self.p += 1;
            }
            while self.t(self.p) == T::NEWLINE {
                self.p += 1;
            }
        }
        self.expect(T::RBRACE)?;
        Ok(properties)
    }

    fn class_suffix(
        &mut self,
        classes: &mut Vec<String>,
        spans: &mut Vec<Span>,
    ) -> Result<(), Error> {
        loop {
            self.p += 1;
            let id = self.expect(T::IDENTIFIER)?;
            classes.push(self.image(id));
            spans.push(self.span(id));
            if self.t(self.p) != T::COMMA {
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
            id: String::new(),
            label: DraftLabel {
                text: String::new(),
                kind: LabelType::Text,
                span: [0, 0],
            },
            span: self.content(k),
            generated: kind != T::IDENTIFIER,
            shape: None,
            metadata: None,
            stereotype: None,
            stereotype_span: None,
            classes: Vec::new(),
            class_spans: Vec::new(),
            explicit: false,
            node_span: [0, 0],
        };
        if kind == T::IDENTIFIER {
            self.p += 1;
            item.id = self.image(k);
            let open = self.t(self.p);
            if open == T::LPAREN || (brackets && open == T::LBRACKET) {
                self.p += 1;
                item.label = self.label()?;
                self.expect(if open == T::LPAREN {
                    T::RPAREN
                } else {
                    T::RBRACKET
                })?;
                item.shape = Some(if open == T::LPAREN {
                    UseCaseShape::Ellipse
                } else {
                    UseCaseShape::Rect
                });
            } else {
                item.label = self.token_label(k);
            }
        } else if kind == T::PLAIN_STRING || kind == T::MARKDOWN_STRING {
            self.p += 1;
            item.label = self.token_label(k);
            item.id = generated_id(&item.label.text);
        } else {
            return Err(self.no_alternative(NAMES));
        }
        Ok(item)
    }

    fn element(&mut self, actor: bool) -> Result<Item, Error> {
        let first = self.p;
        let mut item = self.name(!actor)?;
        if actor {
            item.shape = None;
        }
        if self.t(self.p) == T::METADATA_START {
            item.metadata = Some(self.metadata()?);
        }
        if self.t(self.p) == T::STEREOTYPE_START {
            self.p += 1;
            let text = self.expect(T::STEREOTYPE_TEXT)?;
            self.expect(T::STEREOTYPE_END)?;
            item.stereotype = Some(js::trim(&self.image(text)).to_string());
            item.stereotype_span = Some(self.span(text));
        }
        if self.t(self.p) == T::CLASS_SEPARATOR {
            let (mut classes, mut spans) = (Vec::new(), Vec::new());
            self.class_suffix(&mut classes, &mut spans)?;
            item.classes.extend(classes);
            item.class_spans.extend(spans);
        }
        item.explicit = item.shape.is_some()
            || item.metadata.is_some()
            || item.stereotype.as_deref().is_some_and(|s| !s.is_empty());
        item.node_span = [self.starts[first], self.ends[self.p - 1]];
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

    fn endpoint(item: &Item, declares: bool) -> DraftEndpoint {
        DraftEndpoint {
            named: item.named(),
            declares,
            classed: !item.classes.is_empty(),
        }
    }

    fn occurrence(item: &Item, defines: bool) -> NodeOccurrence {
        NodeOccurrence {
            id: item.id.clone(),
            span: item.node_span,
            id_span: item.span,
            label_span: Some(item.label.span),
            defines: defines.then_some(true),
            stereotype_span: item.stereotype_span,
            metadata: item
                .metadata
                .as_ref()
                .map(|properties| properties.iter().map(DraftProperty::occurrence).collect()),
            class_spans: (!item.class_spans.is_empty()).then(|| item.class_spans.clone()),
        }
    }

    /// Whether a label and then one of the operators that can close it come
    /// next.
    fn labeled_right(&self, closers: u8) -> bool {
        let first = self.t(self.p);
        if !(T::USECASE..=T::MARKDOWN_STRING).contains(&first) {
            return false;
        }
        let mut k = self.p + 1;
        loop {
            let kind = self.t(k);
            if kind == T::MARKERLESS_SOLID || (flags(kind) & closers) != 0 {
                return true;
            }
            if !(T::USECASE..=T::MARKDOWN_STRING).contains(&kind) {
                return false;
            }
            k += 1;
        }
    }

    /// Each dash beyond the second asks for one more rank between the two
    /// ends.
    fn minimum_length(&self, k: usize) -> usize {
        let dashes = self.ends[k] - self.starts[k] - usize::from(self.t(k) != T::MARKERLESS_SOLID);
        dashes.saturating_sub(1).max(1)
    }

    fn relation_tail(&mut self) -> Result<Tail, Error> {
        let first = self.p;
        let mut tail = Tail {
            explicit_id: None,
            explicit_id_span: None,
            kind: RelationshipType::Association,
            arrow_type: Arrow::SOLID_ARROW,
            label: None,
            minlen: 1,
            target: Item {
                id: String::new(),
                label: DraftLabel {
                    text: String::new(),
                    kind: LabelType::Text,
                    span: [0, 0],
                },
                span: [0, 0],
                generated: false,
                shape: None,
                metadata: None,
                stereotype: None,
                stereotype_span: None,
                classes: Vec::new(),
                class_spans: Vec::new(),
                explicit: false,
                node_span: [0, 0],
            },
            span: [0, 0],
        };
        if self.t(self.p) == T::IDENTIFIER && self.t(self.p + 1) == T::AT {
            tail.explicit_id = Some(self.image(self.p));
            tail.explicit_id_span = Some(self.span(self.p));
            self.p += 2;
        }
        let k = self.p;
        let kind = self.t(self.p);
        self.p += 1;
        let width = self.ends[k] - self.starts[k];
        match kind {
            T::DEPENDENCY_ARROW => {
                self.expect(T::COLON)?;
                let word = self.t(self.p);
                if word != T::INCLUDE && word != T::EXTEND {
                    return Err(self.no_alternative("INCLUDE|EXTEND"));
                }
                tail.kind = if word == T::INCLUDE {
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
            T::GENERALIZATION => tail.kind = RelationshipType::Generalization,
            T::FORWARD_SOLID => tail.minlen = self.minimum_length(k),
            T::BACKWARD_SOLID => {
                tail.arrow_type = Arrow::BACK_ARROW;
                // Only the shortest operator can open a label:
                // `A <-- label -- B`.
                if width == 3 && self.labeled_right(0) {
                    tail.label = Some(self.label()?);
                    let close = self.expect(T::MARKERLESS_SOLID)?;
                    tail.minlen = self.minimum_length(close);
                } else {
                    tail.minlen = self.minimum_length(k);
                }
            }
            T::MARKERLESS_SOLID => {
                tail.arrow_type = Arrow::LINE_SOLID;
                if width == 2 && self.labeled_right(CLOSES_LABEL) {
                    tail.label = Some(self.label()?);
                    let close = self.t(self.p);
                    if close == T::FORWARD_SOLID {
                        tail.arrow_type = Arrow::SOLID_ARROW;
                    } else if close == T::FORWARD_CIRCLE {
                        tail.arrow_type = Arrow::CIRCLE_ARROW;
                    } else if close == T::FORWARD_CROSS {
                        tail.arrow_type = Arrow::CROSS_ARROW;
                    } else if close != T::MARKERLESS_SOLID {
                        return Err(self.no_alternative(
                            "FORWARD_SOLID|MARKERLESS_SOLID|FORWARD_CIRCLE|FORWARD_CROSS",
                        ));
                    }
                    if tail.arrow_type == Arrow::SOLID_ARROW || tail.arrow_type == Arrow::LINE_SOLID
                    {
                        tail.minlen = self.minimum_length(self.p);
                    }
                    self.p += 1;
                } else {
                    tail.minlen = self.minimum_length(k);
                }
            }
            T::FORWARD_CIRCLE => tail.arrow_type = Arrow::CIRCLE_ARROW,
            T::FORWARD_CROSS => tail.arrow_type = Arrow::CROSS_ARROW,
            T::BACKWARD_CIRCLE | T::BACKWARD_CROSS => {
                tail.arrow_type = if kind == T::BACKWARD_CIRCLE {
                    Arrow::CIRCLE_ARROW_REVERSED
                } else {
                    Arrow::CROSS_ARROW_REVERSED
                };
                if self.labeled_right(0) {
                    tail.label = Some(self.label()?);
                    self.expect(T::MARKERLESS_SOLID)?;
                }
            }
            _ => {
                self.p -= 1;
                return Err(self.no_alternative(ARROWS));
            }
        }
        tail.target = self.element(false)?;
        tail.span = [self.starts[first], self.ends[self.p - 1]];
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
            source: Self::endpoint(source, declaration),
            target: Self::endpoint(target, target.explicit),
            span: tail.span,
            id: tail.explicit_id.clone(),
            id_span: tail.explicit_id_span,
            kind: tail.kind,
            arrow_type: tail.arrow_type,
            label: tail.label.clone(),
            minlen: tail.minlen,
        });
        nodes.push(Self::occurrence(target, target.explicit));
        let mut out = GraphStatement::new(StatementKind::Edge);
        out.nodes = Some(nodes);
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

    fn declaration(nodes: Vec<NodeOccurrence>) -> GraphStatement {
        let mut out = GraphStatement::new(StatementKind::Node);
        out.nodes = Some(nodes);
        out
    }

    /// `actor A, B` declares several actors; at the top level
    /// `actor A --> B` declares one and relates it.
    fn actors(&mut self, top: bool) -> Result<GraphStatement, Error> {
        self.p += 1;
        let mut items = vec![self.element(true)?];
        let mut tail: Option<Tail> = None;
        if self.t(self.p) == T::COMMA {
            while self.t(self.p) == T::COMMA {
                self.p += 1;
                items.push(self.element(true)?);
            }
        } else if top && (flags(self.t(self.p)) & RELATION) != 0 {
            tail = Some(self.relation_tail()?);
        }
        self.line_end()?;
        for item in &items {
            let draft = self.draft(item, ElementKind::Actor);
            self.drafts.elements.push(draft);
        }
        let nodes: Vec<NodeOccurrence> = items
            .iter()
            .map(|item| Self::occurrence(item, true))
            .collect();
        Ok(match tail {
            Some(tail) => self.relationship(&items[0], true, tail, nodes),
            None => Self::declaration(nodes),
        })
    }

    fn entity(&mut self, top: bool) -> Result<GraphStatement, Error> {
        let mut source = self.element(false)?;
        let tail = if top && (flags(self.t(self.p)) & RELATION) != 0 {
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
        let nodes = vec![Self::occurrence(&source, source.explicit)];
        Ok(match tail {
            Some(tail) => {
                let explicit = source.explicit;
                self.relationship(&source, explicit, tail, nodes)
            }
            None => Self::declaration(nodes),
        })
    }

    fn boundary(&mut self) -> Result<GraphStatement, Error> {
        self.p += 1;
        let mut named = self.name(true)?;
        let inline = if self.t(self.p) == T::METADATA_START {
            Some(self.metadata()?)
        } else {
            None
        };
        if self.t(self.p) == T::CLASS_SEPARATOR {
            let (mut classes, mut spans) = (Vec::new(), Vec::new());
            self.class_suffix(&mut classes, &mut spans)?;
            named.classes.extend(classes);
            named.class_spans.extend(spans);
        }
        let group = DraftBoundary {
            named: named.named(),
            classes: named.classes.clone(),
        };
        self.line_end()?;
        self.drafts.boundaries.push(group.clone());
        self.parent = Some(group.clone());
        let mut children: Vec<GraphStatement> = Vec::new();
        while (flags(self.t(self.p)) & BOUNDARY_LINE) != 0 {
            children.push(self.line(false)?);
        }
        self.parent = None;
        let end = self.expect(T::END)?;
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
        if is_word(kind) || kind == T::CSS_IDENTIFIER {
            self.p += 1;
        } else if kind == T::MARKERLESS_SOLID {
            self.p += 1;
            if !is_word(self.t(self.p)) {
                return Err(self.mismatch("WORD"));
            }
            self.p += 1;
        } else {
            return Err(self.no_alternative("WORD|CSS_IDENTIFIER|MARKERLESS_SOLID"));
        }
        self.expect(T::COLON)?;
        if (flags(self.t(self.p)) & STYLE_PART) == 0 {
            return Err(self.fail(&format!(
                "Expecting: expecting at least one iteration which starts with one of these possible Token sequences::\n  <[{}]>\nbut found: '{}'",
                STYLE_PARTS.replace('|', "] ,["),
                self.image(self.p)
            )));
        }
        while (flags(self.t(self.p)) & STYLE_PART) != 0 {
            self.p += 1;
        }
        // The value is taken from the source, so the spaces inside
        // `1px solid red` survive.
        Ok(self
            .src
            .slice(self.starts[first], self.ends[self.p - 1])
            .replace("\\,", ","))
    }

    fn styles(&mut self) -> Result<Vec<String>, Error> {
        let mut out = vec![self.style()?];
        while self.t(self.p) == T::COMMA {
            self.p += 1;
            out.push(self.style()?);
        }
        Ok(out)
    }

    fn identifiers(&mut self) -> Result<Vec<usize>, Error> {
        let mut out = vec![self.expect(T::IDENTIFIER)?];
        while self.t(self.p) == T::COMMA {
            self.p += 1;
            out.push(self.expect(T::IDENTIFIER)?);
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
        if kind == T::ACC_TITLE_LINE || kind == T::ACC_DESCR_LINE || kind == T::ACC_DESCR_BLOCK {
            self.p += 1;
            let text = self.image(k);
            let after = |c: char| text.find(c).map_or(0, |at| at + 1);
            if kind == T::ACC_DESCR_BLOCK {
                let open = after('{');
                let close = text.rfind('}').unwrap_or(0);
                self.drafts.acc_description =
                    js::trim(text.get(open..close.max(open)).unwrap_or("")).to_string();
            } else if kind == T::ACC_DESCR_LINE {
                self.drafts.acc_description = js::trim(&text[after(':')..]).to_string();
            } else {
                self.drafts.acc_title = js::trim(&text[after(':')..]).to_string();
            }
            out = GraphStatement::new(if kind == T::ACC_TITLE_LINE {
                StatementKind::AccTitle
            } else {
                StatementKind::AccDescr
            });
        } else if kind == T::DIRECTION {
            self.p += 1;
            let value = self.t(self.p);
            if !(T::TD..=T::RL).contains(&value) {
                return Err(self.no_alternative("TD|TB|BT|LR|RL"));
            }
            self.drafts.direction = Some(match value {
                T::TB | T::TD => Direction::TB,
                T::BT => Direction::BT,
                T::LR => Direction::LR,
                _ => Direction::RL,
            });
            self.p += 1;
            out = GraphStatement::new(StatementKind::Direction);
        } else if kind == T::ACTOR {
            return self.actors(true);
        } else if kind == T::SYSTEM_BOUNDARY {
            return self.boundary();
        } else if kind == T::NOTE {
            self.p += 1;
            self.expect(T::FOR)?;
            let target = self.expect(T::IDENTIFIER)?;
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
        } else if kind == T::JSON_DECLARATION_START {
            self.p += 1;
            let literal = self.expect(T::JSON_OBJECT_LITERAL)?;
            let mut classes: Vec<String> = Vec::new();
            let mut class_spans: Vec<Span> = Vec::new();
            if self.t(self.p) == T::CLASS_SEPARATOR {
                self.class_suffix(&mut classes, &mut class_spans)?;
            }
            let mut from = self.starts[k] + 4;
            while matches!(self.src.code(from), 32 | 9) {
                from += 1;
            }
            let mut to = from;
            while crate::util::chars::is_word(self.src.code(to)) {
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
        } else if kind == T::CLASS_DEF {
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
        } else if kind == T::CLASS {
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
        } else if kind == T::STYLE {
            self.p += 1;
            let target = self.expect(T::IDENTIFIER)?;
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
            if next == T::METADATA_START {
                if (flags(self.t(self.p + 2)) & METADATA_FIRST) == 0 {
                    return Err(self.no_alternative(&statement_sequences()));
                }
                self.p += 1;
                let mut target = self.reference(k);
                if kind != T::IDENTIFIER {
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
                let follows = (flags(next) & FOLLOWS_NAME) != 0
                    || (kind == T::IDENTIFIER && (next == T::LPAREN || next == T::LBRACKET));
                // Mermaid turns away the PlantUML statements people are most
                // likely to try.
                if !follows
                    || (kind == T::IDENTIFIER
                        && FORBIDDEN.contains(&self.image(k).to_lowercase().as_str()))
                {
                    return Err(self.no_alternative(&statement_sequences()));
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
        if kind == T::NEWLINE {
            self.p += 1;
            let mut out = GraphStatement::new(StatementKind::Blank);
            out.span = self.span(k);
            return Ok(out);
        }
        if kind == T::COMMENT {
            self.p += 1;
            self.line_end()?;
            let mut out = GraphStatement::new(StatementKind::Comment);
            out.span = self.span(k);
            return Ok(out);
        }
        let mut out = if top {
            self.top_statement()?
        } else if kind == T::ACTOR {
            self.actors(false)?
        } else {
            self.entity(false)?
        };
        out.span = [self.starts[k], self.last_end];
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
    let Tokens {
        types,
        starts,
        ends,
    } = src.tokenize()?;
    let mut p = Parser {
        src: &src,
        types,
        starts,
        ends,
        p: 0,
        last_end: 0,
        drafts: Drafts::default(),
        parent: None,
        pending_json: Vec::new(),
        edge_count: 0,
        note_count: 0,
        top_index: 0,
    };
    let header = p.expect(T::USECASE)?;
    p.line_end()?;
    let mut statements: Vec<GraphStatement> = Vec::new();
    while (flags(p.t(p.p)) & LINE) != 0 {
        p.top_index = statements.len();
        statements.push(p.line(true)?);
    }
    if p.t(p.p) != T::EOF {
        return Err(p.fail(&format!(
            "Redundant input, expecting EOF but found: {}",
            p.image(p.p)
        )));
    }
    // The whole text has to be well formed before the JSON in it is looked
    // at, as in Mermaid.
    for &(json, literal) in &p.pending_json {
        let start = p.starts[literal];
        let parsed = parse_ordered_json(&p.image(literal), || src.line_column(start))?;
        p.drafts.jsons[json].value = parsed.value;
        p.drafts.jsons[json].property_order = parsed.property_order;
    }
    let header_span = p.span(header);
    let drafts = std::mem::take(&mut p.drafts);
    resolve_usecase(&src, drafts, header_span, statements, title)
}
