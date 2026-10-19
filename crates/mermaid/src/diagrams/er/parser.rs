//! Pele's `diagrams/er/parser.ts`.

use super::{
    db::ErModel,
    lexer::{Tokens, tokenize},
    tokens::*,
    types::{Cardinality, DocItem, Identification, ParsedAttribute, RelSpec},
};
use crate::{detect::Kind, error::Error, js};

const DIRECTIONS: [&str; 4] = ["TB", "BT", "RL", "LR"];
const NAME: &str = "'UNICODE_TEXT', 'NUM', 'ENTITY_NAME', 'DECIMAL_NUM', 'ENTITY_ONE'";
const CARDS: &str = "'ZERO_OR_ONE', 'ZERO_OR_MORE', 'ONE_OR_MORE', 'ONLY_ONE', 'MD_PARENT'";
const LINE: &str = "'EOF', 'NEWLINE', 'title', 'acc_title', 'acc_descr', 'acc_descr_multiline_value', 'SUBGRAPH', 'direction_tb', 'direction_bt', 'direction_rl', 'direction_lr', 'CLASSDEF', 'CLASS', 'STYLE', 'UNICODE_TEXT', 'NUM', 'ENTITY_NAME', 'DECIMAL_NUM', 'ENTITY_ONE'";

fn cardinality(t: u8) -> Option<Cardinality> {
    Some(match t {
        ZERO_OR_ONE => Cardinality::ZeroOrOne,
        ZERO_OR_MORE => Cardinality::ZeroOrMore,
        ONE_OR_MORE => Cardinality::OneOrMore,
        ONLY_ONE => Cardinality::OnlyOne,
        MD_PARENT => Cardinality::MdParent,
        _ => return None,
    })
}

fn is_name(t: u8) -> bool {
    matches!(
        t,
        UNICODE_TEXT | ENTITY_NAME | NUM | DECIMAL_NUM | ENTITY_ONE
    )
}

fn is_style_component(t: u8) -> bool {
    matches!(t, STYLE_TEXT | NUM | COLON | BRKT)
}

/// `text` without its first and last characters, which are quotes.
fn unquote(text: &str) -> &str {
    &text[1..text.len() - 1]
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
    tokens: &'t Tokens<'s>,
    src: &'s str,
    i: usize,
}

impl<'s> Parser<'_, 's> {
    fn t(&self) -> u8 {
        self.tokens.types[self.i]
    }

    fn text(&self) -> &'s str {
        self.tokens.texts[self.i]
    }

    fn fail(&self, expected: &str) -> Error {
        let t = self.t();
        let start = self.tokens.starts[self.i];
        if t == ERROR {
            return Error::at(Kind::Er, self.src, start, "", true);
        }
        let got = if t == CHAR {
            self.text()
        } else {
            TOKEN_NAMES[t as usize]
        };
        Error::at(
            Kind::Er,
            self.src,
            start,
            &format!("Expecting {expected}, got '{got}'"),
            false,
        )
    }

    fn expect(&mut self, kind: u8) -> Result<&'s str, Error> {
        if self.t() != kind {
            return Err(self.fail(&format!("'{}'", TOKEN_NAMES[kind as usize])));
        }
        self.i += 1;
        Ok(self.tokens.texts[self.i - 1])
    }

    fn separator(&mut self) -> Result<(), Error> {
        if !matches!(self.t(), NEWLINE | SEMI | EOF) {
            return Err(self.fail("'EOF', 'NEWLINE', 'SEMI'"));
        }
        self.i += 1;
        Ok(())
    }

    fn entity_name(&mut self) -> Result<&'s str, Error> {
        let t = self.t();
        if !is_name(t) {
            return Err(self.fail(NAME));
        }
        let text = self.text();
        self.i += 1;
        Ok(if t == ENTITY_NAME {
            unquote(text)
        } else {
            text
        })
    }

    fn id_list(&mut self) -> Result<Vec<&'s str>, Error> {
        let mut ids = Vec::new();
        loop {
            if !matches!(self.t(), UNICODE_TEXT | STYLE_TEXT) {
                return Err(self.fail("'UNICODE_TEXT', 'STYLE_TEXT'"));
            }
            ids.push(self.text());
            self.i += 1;
            if self.t() != COMMA {
                return Ok(ids);
            }
            self.i += 1;
        }
    }

    fn styles_opt(&mut self) -> Result<Vec<String>, Error> {
        let mut styles = Vec::new();
        loop {
            if !is_style_component(self.t()) {
                return Err(self.fail("'STYLE_TEXT', 'COLON', 'NUM', 'BRKT'"));
            }
            let mut style = self.text().to_string();
            self.i += 1;
            while is_style_component(self.t()) {
                style.push_str(self.text());
                self.i += 1;
            }
            styles.push(style);
            if self.t() != COMMA {
                return Ok(styles);
            }
            self.i += 1;
        }
    }

    fn rel_spec(&mut self) -> Result<RelSpec, Error> {
        let card_b = cardinality(self.t()).unwrap_or(Cardinality::OnlyOne);
        self.i += 1;
        let r = self.t();
        if r != IDENTIFYING && r != NON_IDENTIFYING {
            return Err(self.fail("'NON_IDENTIFYING', 'IDENTIFYING'"));
        }
        let rel_type = if r == IDENTIFYING {
            Identification::Identifying
        } else {
            Identification::NonIdentifying
        };
        self.i += 1;
        let Some(card_a) = cardinality(self.t()) else {
            return Err(self.fail(CARDS));
        };
        self.i += 1;
        Ok(RelSpec {
            card_a,
            rel_type,
            card_b,
        })
    }

    fn role(&mut self) -> Result<&'s str, Error> {
        let t = self.t();
        if !matches!(t, WORD | ENTITY_NAME | UNICODE_TEXT) {
            return Err(self.fail("'UNICODE_TEXT', 'ENTITY_NAME', 'WORD'"));
        }
        let text = self.text();
        self.i += 1;
        Ok(if t == UNICODE_TEXT {
            text
        } else {
            unquote(text)
        })
    }

    fn is_char(&self, ch: &str) -> bool {
        self.t() == CHAR && self.text() == ch
    }

    /// The attributes last to first, as Mermaid's right-recursive rule
    /// builds them.
    fn attributes(&mut self) -> Result<Vec<ParsedAttribute>, Error> {
        let mut list = Vec::new();
        loop {
            let mut kind = self.expect(ATTRIBUTE_WORD)?.to_string();
            if self.is_char("?") {
                kind.push_str(self.text());
                self.i += 1;
            }
            if self.t() != ATTRIBUTE_WORD {
                return Err(self.fail(if kind.ends_with('?') {
                    "'ATTRIBUTE_WORD'"
                } else {
                    "'ATTRIBUTE_WORD', '?'"
                }));
            }
            let mut attribute = ParsedAttribute {
                kind,
                name: self.text().to_string(),
                keys: None,
                comment: None,
            };
            self.i += 1;
            if self.t() == ATTRIBUTE_KEY {
                let mut keys = vec![self.text().to_string()];
                self.i += 1;
                while self.is_char(",") {
                    self.i += 1;
                    keys.push(self.expect(ATTRIBUTE_KEY)?.to_string());
                }
                attribute.keys = Some(keys);
            }
            if self.t() == COMMENT {
                attribute.comment = Some(unquote(self.text()).to_string());
                self.i += 1;
            }
            list.push(attribute);
            if self.t() == BLOCK_STOP {
                break;
            }
        }
        list.reverse();
        Ok(list)
    }

    /// An optional `{ ... }` block; `None` when there is none or it is
    /// empty.
    fn block(&mut self) -> Result<Option<Vec<ParsedAttribute>>, Error> {
        if self.t() != BLOCK_START {
            return Ok(None);
        }
        self.i += 1;
        if self.t() == BLOCK_STOP {
            self.i += 1;
            return Ok(None);
        }
        let list = self.attributes()?;
        self.i += 1;
        Ok(Some(list))
    }

    fn entity_statement(&mut self, db: &mut ErModel) -> Result<Vec<&'s str>, Error> {
        let a = self.entity_name()?;
        let mut t = self.t();
        if t == SQS {
            self.i += 1;
            let alias = self.entity_name()?;
            self.expect(SQE)?;
            let mut classes = None;
            if self.t() == STYLE_SEPARATOR {
                self.i += 1;
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
        if t == STYLE_SEPARATOR {
            self.i += 1;
            classes = Some(self.id_list()?);
            t = self.t();
        }
        if cardinality(t).is_some() {
            let spec = self.rel_spec()?;
            let b = self.entity_name()?;
            let mut classes_b = None;
            if self.t() == STYLE_SEPARATOR {
                self.i += 1;
                classes_b = Some(self.id_list()?);
            }
            self.expect(COLON)?;
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
}

pub fn parse_er(src: &str, db: &mut ErModel) -> Result<(), Error> {
    let tokens = tokenize(src);
    let mut r = Parser {
        tokens: &tokens,
        src,
        i: 0,
    };

    r.expect(ER_DIAGRAM)?;
    // Subgraphs nest on this stack rather than on the call stack; the first
    // frame is the diagram itself.
    let mut frames = vec![Frame {
        id: String::new(),
        title: String::new(),
        list: Vec::new(),
    }];
    let nested = |frames: &Vec<Frame>| frames.len() > 1;

    loop {
        let t = r.t();
        match t {
            NEWLINE => r.i += 1,
            EOF => {
                r.i += 1;
                if nested(&frames) {
                    return Err(r.fail(&format!("'END', {LINE}")));
                }
                return Ok(());
            }
            END_KW => {
                if !nested(&frames) {
                    return Err(r.fail(LINE));
                }
                r.i += 1;
                let frame = frames.pop().unwrap_or_else(|| unreachable!());
                let id = db.add_sub_graph(&frame.id, &frame.list, &frame.title);
                if nested(&frames)
                    && let Some(parent) = frames.last_mut()
                {
                    parent.list.push(DocItem::Name(id));
                }
            }
            SUBGRAPH => {
                r.i += 1;
                let id = r.entity_name()?;
                let mut title = id.to_string();
                if r.t() == SQS {
                    r.i += 1;
                    title = r.entity_name()?.to_string();
                    while is_name(r.t()) {
                        title.push(' ');
                        title.push_str(r.entity_name()?);
                    }
                    r.expect(SQE)?;
                }
                r.separator()?;
                frames.push(Frame {
                    id: id.to_string(),
                    title,
                    list: Vec::new(),
                });
            }
            ACC_TITLE | ACC_DESCR | ACC_DESCR_MULTILINE_VALUE => {
                let value = if t == ACC_DESCR_MULTILINE_VALUE {
                    let text = r.text();
                    r.i += 1;
                    js::trim(text)
                } else {
                    r.i += 1;
                    js::trim(r.expect(if t == ACC_TITLE {
                        ACC_TITLE_VALUE
                    } else {
                        ACC_DESCR_VALUE
                    })?)
                };
                if t == ACC_TITLE {
                    db.set_acc_title(value);
                } else {
                    db.set_acc_description(value);
                }
                push_name(&mut frames, value);
            }
            DIRECTION_TB | DIRECTION_BT | DIRECTION_RL | DIRECTION_LR => {
                r.i += 1;
                let value = DIRECTIONS[(t - DIRECTION_TB) as usize];
                if nested(&frames) {
                    if let Some(frame) = frames.last_mut() {
                        frame.list.push(DocItem::Dir(value));
                    }
                } else {
                    db.set_direction(value);
                }
            }
            CLASSDEF | STYLE => {
                let keyword = r.text();
                r.i += 1;
                let ids = r.id_list()?;
                let styles = r.styles_opt()?;
                r.separator()?;
                if t == CLASSDEF {
                    db.add_class(&ids, &styles);
                } else {
                    db.add_css_styles(&ids, &styles);
                }
                push_name(&mut frames, keyword);
            }
            CLASS => {
                let keyword = r.text();
                r.i += 1;
                let ids = r.id_list()?;
                let names = r.id_list()?;
                db.set_class(&ids, &names);
                push_name(&mut frames, keyword);
            }
            _ => {
                if !is_name(t) {
                    return Err(r.fail(&if nested(&frames) {
                        format!("'END', {LINE}")
                    } else {
                        LINE.to_string()
                    }));
                }
                let names = r.entity_statement(db)?;
                for name in names {
                    push_name(&mut frames, name);
                }
            }
        }
    }
}

/// Lists `value` in the subgraph being read, if any.
fn push_name(frames: &mut [Frame], value: &str) {
    if frames.len() > 1
        && let Some(frame) = frames.last_mut()
    {
        frame.list.push(DocItem::Name(value.to_string()));
    }
}
