//! Pele's `diagrams/er/parser.ts`.

use super::{
    db::ErModel,
    lexer::tokenize,
    tokens::T,
    types::{Cardinality, DocItem, Identification, ParsedAttribute, RelSpec},
};
use crate::util::chars::inner;
use crate::{detect::Kind, error::Error, geom::Direction, js, token::Stream};

const NAME: &str = "'UNICODE_TEXT', 'NUM', 'ENTITY_NAME', 'DECIMAL_NUM', 'ENTITY_ONE'";
const CARDS: &str = "'ZERO_OR_ONE', 'ZERO_OR_MORE', 'ONE_OR_MORE', 'ONLY_ONE', 'MD_PARENT'";
const LINE: &str = "'EOF', 'NEWLINE', 'title', 'acc_title', 'acc_descr', 'acc_descr_multiline_value', 'SUBGRAPH', 'direction_tb', 'direction_bt', 'direction_rl', 'direction_lr', 'CLASSDEF', 'CLASS', 'STYLE', 'UNICODE_TEXT', 'NUM', 'ENTITY_NAME', 'DECIMAL_NUM', 'ENTITY_ONE'";

impl T {
    fn cardinality(self) -> Option<Cardinality> {
        Some(match self {
            T::ZeroOrOne => Cardinality::ZeroOrOne,
            T::ZeroOrMore => Cardinality::ZeroOrMore,
            T::OneOrMore => Cardinality::OneOrMore,
            T::OnlyOne => Cardinality::OnlyOne,
            T::MdParent => Cardinality::MdParent,
            _ => return None,
        })
    }

    fn is_name(self) -> bool {
        matches!(
            self,
            T::UnicodeText | T::EntityName | T::Num | T::DecimalNum | T::EntityOne
        )
    }

    fn is_style_component(self) -> bool {
        matches!(self, T::StyleText | T::Num | T::Colon | T::Brkt)
    }
}

struct Frame {
    id: String,
    title: String,
    /// What Mermaid's grammar hands to addSubGraph: the value of every
    /// statement inside. Besides entity names that includes accessibility
    /// text and the keywords `style`, `class` and `classDef`.
    list: Vec<DocItem>,
}

struct Parser<'t, 's> {
    s: Stream<'t, 's, T>,
    /// The subgraphs being read, innermost last.
    open: Vec<Frame>,
}

impl<'t> Parser<'t, '_> {
    fn separator(&mut self) -> Result<(), Error> {
        if !self.s.is_any(&[T::Newline, T::Semi, T::Eof]) {
            return Err(self.s.fail("'EOF', 'NEWLINE', 'SEMI'"));
        }
        self.s.bump();
        Ok(())
    }

    fn entity_name(&mut self) -> Result<&'t str, Error> {
        let t = self.s.kind();
        if !t.is_name() {
            return Err(self.s.fail(NAME));
        }
        let text = self.s.take();
        Ok(if t == T::EntityName {
            inner(text)
        } else {
            text
        })
    }

    fn id_list(&mut self) -> Result<Vec<&'t str>, Error> {
        let mut ids = Vec::new();
        loop {
            if !self.s.is_any(&[T::UnicodeText, T::StyleText]) {
                return Err(self.s.fail("'UNICODE_TEXT', 'STYLE_TEXT'"));
            }
            ids.push(self.s.take());
            if !self.s.eat(T::Comma) {
                return Ok(ids);
            }
        }
    }

    fn styles_opt(&mut self) -> Result<Vec<String>, Error> {
        let mut styles = Vec::new();
        loop {
            if !self.s.kind().is_style_component() {
                return Err(self.s.fail("'STYLE_TEXT', 'COLON', 'NUM', 'BRKT'"));
            }
            let mut style = self.s.take().to_string();
            while self.s.kind().is_style_component() {
                style.push_str(self.s.take());
            }
            styles.push(style);
            if !self.s.eat(T::Comma) {
                return Ok(styles);
            }
        }
    }

    fn rel_spec(&mut self) -> Result<RelSpec, Error> {
        let card_b = self.s.kind().cardinality().unwrap_or(Cardinality::OnlyOne);
        self.s.bump();
        let r = self.s.kind();
        if r != T::Identifying && r != T::NonIdentifying {
            return Err(self.s.fail("'NON_IDENTIFYING', 'IDENTIFYING'"));
        }
        let rel_type = if r == T::Identifying {
            Identification::Identifying
        } else {
            Identification::NonIdentifying
        };
        self.s.bump();
        let Some(card_a) = self.s.kind().cardinality() else {
            return Err(self.s.fail(CARDS));
        };
        self.s.bump();
        Ok(RelSpec {
            card_a,
            rel_type,
            card_b,
        })
    }

    fn role(&mut self) -> Result<&'t str, Error> {
        let t = self.s.kind();
        if !matches!(t, T::Word | T::EntityName | T::UnicodeText) {
            return Err(self.s.fail("'UNICODE_TEXT', 'ENTITY_NAME', 'WORD'"));
        }
        let text = self.s.take();
        Ok(if t == T::UnicodeText {
            text
        } else {
            inner(text)
        })
    }

    fn is_char(&self, ch: &str) -> bool {
        self.s.is(T::Char) && self.s.text() == ch
    }

    /// The attributes last to first, as Mermaid's right-recursive rule
    /// builds them.
    fn attributes(&mut self) -> Result<Vec<ParsedAttribute>, Error> {
        let mut list = Vec::new();
        loop {
            let mut kind = self.s.expect(T::AttributeWord, "")?.to_string();
            if self.is_char("?") {
                kind.push_str(self.s.take());
            }
            if !self.s.is(T::AttributeWord) {
                return Err(self.s.fail(if kind.ends_with('?') {
                    "'ATTRIBUTE_WORD'"
                } else {
                    "'ATTRIBUTE_WORD', '?'"
                }));
            }
            let mut attribute = ParsedAttribute {
                kind,
                name: self.s.text().to_string(),
                keys: None,
                comment: None,
            };
            self.s.bump();
            if self.s.is(T::AttributeKey) {
                let mut keys = vec![self.s.take().to_string()];
                while self.is_char(",") {
                    self.s.bump();
                    keys.push(self.s.expect(T::AttributeKey, "")?.to_string());
                }
                attribute.keys = Some(keys);
            }
            if self.s.is(T::Comment) {
                attribute.comment = Some(inner(self.s.take()).to_string());
            }
            list.push(attribute);
            if self.s.is(T::BlockStop) {
                break;
            }
        }
        list.reverse();
        Ok(list)
    }

    /// An optional `{ ... }` block; `None` when there is none or it is
    /// empty.
    fn block(&mut self) -> Result<Option<Vec<ParsedAttribute>>, Error> {
        if !self.s.eat(T::BlockStart) {
            return Ok(None);
        }
        if self.s.eat(T::BlockStop) {
            return Ok(None);
        }
        let list = self.attributes()?;
        self.s.bump();
        Ok(Some(list))
    }

    fn entity_statement(&mut self, db: &mut ErModel) -> Result<Vec<&'t str>, Error> {
        let a = self.entity_name()?;
        let mut t = self.s.kind();
        if t == T::Sqs {
            self.s.bump();
            let alias = self.entity_name()?;
            self.s.expect(T::Sqe, "")?;
            let mut classes = None;
            if self.s.eat(T::StyleSeparator) {
                classes = Some(self.id_list()?);
            }
            let attrs = self.block()?;
            db.add_entity(a, alias);
            if let Some(attrs) = attrs {
                db.add_attributes(a, attrs);
            }
            if let Some(classes) = classes {
                db.set_class(&[a], &classes);
            }
            return Ok(vec![a]);
        }
        let mut classes = None;
        if t == T::StyleSeparator {
            self.s.bump();
            classes = Some(self.id_list()?);
            t = self.s.kind();
        }
        if t.cardinality().is_some() {
            let spec = self.rel_spec()?;
            let b = self.entity_name()?;
            let mut classes_b = None;
            if self.s.eat(T::StyleSeparator) {
                classes_b = Some(self.id_list()?);
            }
            self.s.expect(T::Colon, "")?;
            let label = self.role()?;
            db.add_entity(a, "");
            db.add_entity(b, "");
            db.add_relationship(a, label, b, spec);
            if let Some(classes) = classes {
                db.set_class(&[a], &classes);
            }
            if let Some(classes) = classes_b {
                db.set_class(&[b], &classes);
            }
            return Ok(vec![a, b]);
        }
        let attrs = self.block()?;
        db.add_entity(a, "");
        if let Some(attrs) = attrs {
            db.add_attributes(a, attrs);
        }
        if let Some(classes) = classes {
            db.set_class(&[a], &classes);
        }
        Ok(vec![a])
    }

    /// Lists `value` in the subgraph being read, if any.
    fn list(&mut self, value: &str) {
        if let Some(frame) = self.open.last_mut() {
            frame.list.push(DocItem::Name(value.to_string()));
        }
    }

    /// What a line may hold here, for an error.
    fn line_expected(&self) -> String {
        if self.open.is_empty() {
            LINE.to_string()
        } else {
            format!("'END', {LINE}")
        }
    }
}

pub fn parse_er(src: &str, db: &mut ErModel) -> Result<(), Error> {
    let tokens = tokenize(src);
    let mut r = Parser {
        s: Stream::new(Kind::Er, src, &tokens),
        open: Vec::new(),
    };

    r.s.expect(T::ErDiagram, "")?;
    loop {
        let t = r.s.kind();
        match t {
            T::Newline => r.s.bump(),
            T::Eof => {
                r.s.bump();
                if !r.open.is_empty() {
                    return Err(r.s.fail(&r.line_expected()));
                }
                return Ok(());
            }
            T::EndKw => {
                let Some(frame) = r.open.pop() else {
                    return Err(r.s.fail(LINE));
                };
                r.s.bump();
                let id = db.add_sub_graph(&frame.id, &frame.list, &frame.title);
                if let Some(parent) = r.open.last_mut() {
                    parent.list.push(DocItem::Name(id));
                }
            }
            T::Subgraph => {
                r.s.bump();
                let id = r.entity_name()?;
                let mut title = id.to_string();
                if r.s.eat(T::Sqs) {
                    title = r.entity_name()?.to_string();
                    while r.s.kind().is_name() {
                        title.push(' ');
                        title.push_str(r.entity_name()?);
                    }
                    r.s.expect(T::Sqe, "")?;
                }
                r.separator()?;
                r.open.push(Frame {
                    id: id.to_string(),
                    title,
                    list: Vec::new(),
                });
            }
            T::AccTitle | T::AccDescr | T::AccDescrMultilineValue => {
                let value = if t == T::AccDescrMultilineValue {
                    js::trim(r.s.take())
                } else {
                    r.s.bump();
                    js::trim(r.s.expect(
                        if t == T::AccTitle {
                            T::AccTitleValue
                        } else {
                            T::AccDescrValue
                        },
                        "",
                    )?)
                };
                if t == T::AccTitle {
                    db.titles.set_acc_title(value);
                } else {
                    db.titles.set_acc_descr(value);
                }
                r.list(value);
            }
            T::DirectionTb | T::DirectionBt | T::DirectionRl | T::DirectionLr => {
                r.s.bump();
                let value = match t {
                    T::DirectionTb => Direction::TopToBottom,
                    T::DirectionBt => Direction::BottomToTop,
                    T::DirectionRl => Direction::RightToLeft,
                    _ => Direction::LeftToRight,
                };
                match r.open.last_mut() {
                    Some(frame) => frame.list.push(DocItem::Dir(value)),
                    None => db.direction = value,
                }
            }
            T::Classdef | T::Style => {
                let keyword = r.s.take();
                let ids = r.id_list()?;
                let styles = r.styles_opt()?;
                r.separator()?;
                if t == T::Classdef {
                    db.add_class(&ids, &styles);
                } else {
                    db.add_css_styles(&ids, &styles);
                }
                r.list(keyword);
            }
            T::Class => {
                let keyword = r.s.take();
                let ids = r.id_list()?;
                let names = r.id_list()?;
                db.set_class(&ids, &names);
                r.list(keyword);
            }
            _ => {
                if !t.is_name() {
                    return Err(r.s.fail(&r.line_expected()));
                }
                let names = r.entity_statement(db)?;
                for name in names {
                    r.list(name);
                }
            }
        }
    }
}
