//! Pele's `diagrams/c4/parser.ts`.

use super::{
    db::C4Model,
    lexer::tokenize,
    tokens::*,
    types::{C4Attr, C4Named},
};
use crate::{detect::Kind, error::Error, js, util::chars::width_at};

/// Mermaid's typeC4Shape for each shape keyword, indexed from `PERSON`.
const KINDS: [&str; 20] = [
    "person",
    "external_person",
    "system",
    "system_db",
    "system_queue",
    "external_system",
    "external_system_db",
    "external_system_queue",
    "container",
    "container_db",
    "container_queue",
    "external_container",
    "external_container_db",
    "external_container_queue",
    "component",
    "component_db",
    "component_queue",
    "external_component",
    "external_component_db",
    "external_component_queue",
];
const REL_TYPES: [&str; 8] = [
    "rel", "birel", "rel_u", "rel_d", "rel_l", "rel_r", "rel_b", "rel",
];
const NODE_TYPES: [&str; 3] = ["node", "nodeL", "nodeR"];

const HEADERS: &str = "'direction_tb', 'direction_bt', 'direction_rl', 'direction_lr', 'C4_CONTEXT', 'C4_CONTAINER', 'C4_COMPONENT', 'C4_DYNAMIC', 'C4_DEPLOYMENT'";
const ATTRIBUTES: &str = "'STR', 'STR_KEY', 'ATTRIBUTE', 'ATTRIBUTE_EMPTY'";

#[derive(Clone, Copy, PartialEq, Eq)]
enum P {
    /// A statement is required.
    Item,
    /// Only a diagram statement may come next: the first one in a boundary,
    /// or one written straight after a title.
    Diagram,
    ItemOrEnd,
    AfterOther,
    AfterDiagram,
}

fn is_other(t: u8) -> bool {
    t == TITLE
        || t == ACC_DESCRIPTION
        || t == ACC_TITLE
        || t == ACC_DESCR
        || t == ACC_DESCR_MULTILINE_VALUE
}

fn is_diagram(t: u8) -> bool {
    (PERSON..=UPDATE_LAYOUT_CONFIG).contains(&t)
}

/// `text` past its first `keyword` bytes and the space after them, as
/// `substring(keyword + 1)` reads it.
fn after_keyword(text: &str, keyword: usize) -> &str {
    &text[keyword + width_at(text, keyword)..]
}

struct Parser<'a> {
    src: &'a str,
    types: Vec<u8>,
    starts: Vec<usize>,
    ends: Vec<usize>,
    i: usize,
}

impl Parser<'_> {
    fn ty(&self) -> u8 {
        self.types.get(self.i).copied().unwrap_or(END)
    }

    fn text(&self, at: usize) -> &str {
        &self.src[self.starts[at]..self.ends[at]]
    }

    fn fail(&self, expected: &str) -> Error {
        let t = self.ty();
        let at = self.starts.get(self.i).copied().unwrap_or(self.src.len());
        if t == ERROR {
            return Error::at(Kind::C4, self.src, at, "", true);
        }
        Error::at(
            Kind::C4,
            self.src,
            at,
            &format!("Expecting {expected}, got '{}'", TOKEN_NAMES[t as usize]),
            false,
        )
    }

    fn expect(&mut self, ty: u8) -> Result<(), Error> {
        if self.ty() != ty {
            return Err(self.fail(&format!("'{}'", TOKEN_NAMES[ty as usize])));
        }
        self.i += 1;
        Ok(())
    }

    fn attributes(&mut self) -> Result<Vec<C4Attr>, Error> {
        let mut attrs = Vec::new();
        loop {
            let t = self.ty();
            if t == STR {
                attrs.push(C4Attr::Str(js::trim(self.text(self.i)).to_string()));
                self.i += 1;
            } else if t == ATTRIBUTE_EMPTY {
                attrs.push(C4Attr::Str(String::new()));
                self.i += 1;
            } else if t == STR_KEY {
                self.i += 1;
                if self.ty() != STR_VALUE {
                    return Err(self.fail("'STR_VALUE'"));
                }
                attrs.push(C4Attr::Named(C4Named {
                    key: js::trim(self.text(self.i - 1)).to_string(),
                    value: js::trim(self.text(self.i)).to_string(),
                }));
                self.i += 1;
            } else {
                break;
            }
        }
        if attrs.is_empty() {
            return Err(self.fail(ATTRIBUTES));
        }
        Ok(attrs)
    }

    fn other(&mut self, t: u8, db: &mut C4Model) -> Result<(), Error> {
        if t == TITLE {
            db.set_title(after_keyword(self.text(self.i), 5));
            self.i += 1;
        } else if t == ACC_DESCRIPTION {
            db.set_acc_description(after_keyword(self.text(self.i), 14));
            self.i += 1;
        } else if t == ACC_DESCR_MULTILINE_VALUE {
            db.set_acc_description(js::trim(self.text(self.i)));
            self.i += 1;
        } else {
            self.i += 1;
            if t == ACC_TITLE {
                if self.ty() != ACC_TITLE_VALUE {
                    return Err(self.fail("'acc_title_value'"));
                }
                // The grammar hands `accTitle:` to setTitle, so it becomes the
                // visible title.
                db.set_title(js::trim(self.text(self.i)));
            } else {
                if self.ty() != ACC_DESCR_VALUE {
                    return Err(self.fail("'acc_descr_value'"));
                }
                db.set_acc_description(js::trim(self.text(self.i)));
            }
            self.i += 1;
        }
        Ok(())
    }

    /// True when the statement opens a boundary.
    fn diagram(&mut self, t: u8, db: &mut C4Model) -> Result<bool, Error> {
        self.i += 1;
        let mut attrs = self.attributes()?;
        if t <= SYSTEM_EXT_QUEUE {
            db.add_person_or_system(KINDS[(t - PERSON) as usize], &attrs);
        } else if t <= CONTAINER_EXT_QUEUE {
            db.add_container(KINDS[(t - PERSON) as usize], &attrs);
        } else if t <= COMPONENT_EXT_QUEUE {
            db.add_component(KINDS[(t - PERSON) as usize], &attrs);
        } else if t <= NODE_R {
            let at = attrs.len().min(2);
            if t == ENTERPRISE_BOUNDARY {
                attrs.insert(at, C4Attr::Str("ENTERPRISE".into()));
            } else if t == SYSTEM_BOUNDARY {
                attrs.insert(at, C4Attr::Str("SYSTEM".into()));
            } else if t == CONTAINER_BOUNDARY {
                attrs.insert(at, C4Attr::Str("CONTAINER".into()));
            }
            if t >= NODE {
                db.add_deployment_node(NODE_TYPES[(t - NODE) as usize], &attrs);
            } else if t == CONTAINER_BOUNDARY {
                db.add_container_boundary(&attrs);
            } else {
                db.add_person_or_system_boundary(&attrs);
            }
            if self.ty() == LBRACE {
                self.i += 1;
                self.expect(NEWLINE)?;
            } else if self.ty() == NEWLINE {
                self.i += 1;
                self.expect(LBRACE)?;
                if self.ty() == NEWLINE {
                    self.i += 1;
                }
            } else {
                return Err(self.fail("'NEWLINE', 'LBRACE'"));
            }
            return Ok(true);
        } else if t <= REL_INDEX {
            // RelIndex takes a sequence number first, which Mermaid ignores.
            if t == REL_INDEX {
                attrs.remove(0);
            }
            db.add_rel(REL_TYPES[(t - REL) as usize], &attrs);
        } else if t == UPDATE_EL_STYLE {
            db.update_el_style("update_el_style", &attrs);
        } else if t == UPDATE_REL_STYLE {
            db.update_rel_style("update_rel_style", &attrs);
        } else {
            db.update_layout_config("update_layout_config", &attrs);
        }
        Ok(false)
    }
}

/// Accepts what the parser Jison generates from c4Diagram.jison accepts, and
/// calls the model builder in the order its semantic actions do. Boundaries
/// nest through a counter, not recursion.
pub fn parse_c4(src: &str, db: &mut C4Model) -> Result<(), Error> {
    let tokens = tokenize(src);
    let mut r = Parser {
        src,
        types: tokens.types,
        starts: tokens.starts,
        ends: tokens.ends,
        i: 0,
    };

    let header = r.ty();
    if !(C4_CONTEXT..=C4_DEPLOYMENT).contains(&header) {
        return Err(r.fail(HEADERS));
    }
    r.i += 1;
    r.expect(NEWLINE)?;

    let mut depth = 0usize;
    let mut state = P::Item;
    loop {
        let t = r.ty();
        if state == P::AfterOther || state == P::AfterDiagram {
            if t == NEWLINE {
                r.i += 1;
                state = P::ItemOrEnd;
                continue;
            }
            if state == P::AfterOther && is_diagram(t) {
                state = P::Diagram;
                continue;
            }
        } else if is_diagram(t) {
            if r.diagram(t, db)? {
                depth += 1;
                state = P::Diagram;
            } else {
                state = P::AfterDiagram;
            }
            continue;
        } else if is_other(t) && state != P::Diagram {
            r.other(t, db)?;
            state = P::AfterOther;
            continue;
        } else if state != P::ItemOrEnd {
            return Err(r.fail(
                "'title', 'accDescription', 'acc_title', 'acc_descr', 'acc_descr_multiline_value', 'PERSON', 'REL'",
            ));
        }

        if depth > 0 {
            if t != RBRACE {
                return Err(r.fail("'NEWLINE', 'RBRACE'"));
            }
            r.i += 1;
            db.pop_boundary_parse_stack();
            depth -= 1;
            state = P::AfterDiagram;
        } else {
            if t != EOF {
                return Err(r.fail("'NEWLINE', 'EOF'"));
            }
            r.i += 1;
            r.expect(END)?;
            db.set_c4_type(r.text(0));
            return Ok(());
        }
    }
}
