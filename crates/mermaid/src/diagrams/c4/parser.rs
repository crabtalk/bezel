//! Pele's `diagrams/c4/parser.ts`.

use super::{
    db::C4Model,
    lexer::tokenize,
    tokens::T,
    types::{C4Attr, C4Named},
};
use crate::{detect::Kind, error::Error, js, token::Stream, util::chars::width_at};

const HEADER: [T; 5] = [
    T::C4Context,
    T::C4Container,
    T::C4Component,
    T::C4Dynamic,
    T::C4Deployment,
];

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

fn is_other(t: T) -> bool {
    matches!(
        t,
        T::Title | T::AccDescription | T::AccTitle | T::AccDescr | T::AccDescrMultilineValue
    )
}

/// What a diagram statement adds to the model.
#[derive(Clone, Copy)]
enum Statement {
    /// Each element with Mermaid's typeC4Shape for it.
    PersonOrSystem(&'static str),
    Container(&'static str),
    Component(&'static str),
    /// The type Mermaid inserts as the third attribute, if any.
    Boundary(Option<&'static str>),
    ContainerBoundary,
    DeploymentNode(&'static str),
    Rel(&'static str),
    /// Takes a sequence number first, which Mermaid ignores.
    RelIndex,
    UpdateElStyle,
    UpdateRelStyle,
    UpdateLayoutConfig,
}

fn statement(t: T) -> Option<Statement> {
    use Statement::*;
    Some(match t {
        T::Person => PersonOrSystem("person"),
        T::PersonExt => PersonOrSystem("external_person"),
        T::System => PersonOrSystem("system"),
        T::SystemDb => PersonOrSystem("system_db"),
        T::SystemQueue => PersonOrSystem("system_queue"),
        T::SystemExt => PersonOrSystem("external_system"),
        T::SystemExtDb => PersonOrSystem("external_system_db"),
        T::SystemExtQueue => PersonOrSystem("external_system_queue"),
        T::Container => Container("container"),
        T::ContainerDb => Container("container_db"),
        T::ContainerQueue => Container("container_queue"),
        T::ContainerExt => Container("external_container"),
        T::ContainerExtDb => Container("external_container_db"),
        T::ContainerExtQueue => Container("external_container_queue"),
        T::Component => Component("component"),
        T::ComponentDb => Component("component_db"),
        T::ComponentQueue => Component("component_queue"),
        T::ComponentExt => Component("external_component"),
        T::ComponentExtDb => Component("external_component_db"),
        T::ComponentExtQueue => Component("external_component_queue"),
        T::EnterpriseBoundary => Boundary(Some("ENTERPRISE")),
        T::SystemBoundary => Boundary(Some("SYSTEM")),
        T::Boundary => Boundary(None),
        T::ContainerBoundary => ContainerBoundary,
        T::Node => DeploymentNode("node"),
        T::NodeL => DeploymentNode("nodeL"),
        T::NodeR => DeploymentNode("nodeR"),
        T::Rel => Rel("rel"),
        T::Birel => Rel("birel"),
        T::RelU => Rel("rel_u"),
        T::RelD => Rel("rel_d"),
        T::RelL => Rel("rel_l"),
        T::RelR => Rel("rel_r"),
        T::RelB => Rel("rel_b"),
        T::RelIndex => RelIndex,
        T::UpdateElStyle => UpdateElStyle,
        T::UpdateRelStyle => UpdateRelStyle,
        T::UpdateLayoutConfig => UpdateLayoutConfig,
        _ => return None,
    })
}

/// `text` past its first `keyword` bytes and the space after them, as
/// `substring(keyword + 1)` reads it.
fn after_keyword(text: &str, keyword: usize) -> &str {
    &text[keyword + width_at(text, keyword)..]
}

struct Parser<'t, 's> {
    s: Stream<'t, 's, T>,
}

impl Parser<'_, '_> {
    fn attributes(&mut self) -> Result<Vec<C4Attr>, Error> {
        let mut attrs = Vec::new();
        loop {
            match self.s.kind() {
                T::Str => attrs.push(C4Attr::Str(js::trim(self.s.take()).to_string())),
                T::AttributeEmpty => {
                    self.s.bump();
                    attrs.push(C4Attr::Str(String::new()));
                }
                T::StrKey => {
                    let key = js::trim(self.s.take()).to_string();
                    if !self.s.is(T::StrValue) {
                        return Err(self.s.fail("'STR_VALUE'"));
                    }
                    attrs.push(C4Attr::Named(C4Named {
                        key,
                        value: js::trim(self.s.take()).to_string(),
                    }));
                }
                _ => break,
            }
        }
        if attrs.is_empty() {
            return Err(self.s.fail(ATTRIBUTES));
        }
        Ok(attrs)
    }

    fn other(&mut self, t: T, db: &mut C4Model) -> Result<(), Error> {
        match t {
            T::Title => db.set_title(after_keyword(self.s.take(), 5)),
            T::AccDescription => db.set_acc_description(after_keyword(self.s.take(), 14)),
            T::AccDescrMultilineValue => db.set_acc_description(js::trim(self.s.take())),
            T::AccTitle => {
                self.s.bump();
                if !self.s.is(T::AccTitleValue) {
                    return Err(self.s.fail("'acc_title_value'"));
                }
                // The grammar hands `accTitle:` to setTitle, so it becomes the
                // visible title.
                db.set_title(js::trim(self.s.take()));
            }
            _ => {
                self.s.bump();
                if !self.s.is(T::AccDescrValue) {
                    return Err(self.s.fail("'acc_descr_value'"));
                }
                db.set_acc_description(js::trim(self.s.take()));
            }
        }
        Ok(())
    }

    fn open_boundary(&mut self) -> Result<(), Error> {
        if self.s.eat(T::Lbrace) {
            self.s.expect(T::Newline, "")?;
        } else if self.s.eat(T::Newline) {
            self.s.expect(T::Lbrace, "")?;
            self.s.eat(T::Newline);
        } else {
            return Err(self.s.fail("'NEWLINE', 'LBRACE'"));
        }
        Ok(())
    }

    /// True when the statement opens a boundary.
    fn diagram(&mut self, statement: Statement, db: &mut C4Model) -> Result<bool, Error> {
        self.s.bump();
        let mut attrs = self.attributes()?;
        match statement {
            Statement::PersonOrSystem(kind) => db.add_person_or_system(kind, &attrs),
            Statement::Container(kind) => db.add_container(kind, &attrs),
            Statement::Component(kind) => db.add_component(kind, &attrs),
            Statement::Boundary(kind) => {
                if let Some(kind) = kind {
                    attrs.insert(attrs.len().min(2), C4Attr::Str(kind.into()));
                }
                db.add_person_or_system_boundary(&attrs);
                self.open_boundary()?;
                return Ok(true);
            }
            Statement::ContainerBoundary => {
                attrs.insert(attrs.len().min(2), C4Attr::Str("CONTAINER".into()));
                db.add_container_boundary(&attrs);
                self.open_boundary()?;
                return Ok(true);
            }
            Statement::DeploymentNode(kind) => {
                db.add_deployment_node(kind, &attrs);
                self.open_boundary()?;
                return Ok(true);
            }
            Statement::Rel(kind) => db.add_rel(kind, &attrs),
            Statement::RelIndex => {
                attrs.remove(0);
                db.add_rel("rel", &attrs);
            }
            Statement::UpdateElStyle => db.update_el_style("update_el_style", &attrs),
            Statement::UpdateRelStyle => db.update_rel_style("update_rel_style", &attrs),
            Statement::UpdateLayoutConfig => {
                db.update_layout_config("update_layout_config", &attrs)
            }
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
        s: Stream::new(Kind::C4, src, &tokens),
    };

    if !r.s.is_any(&HEADER) {
        return Err(r.s.fail(HEADERS));
    }
    let header = r.s.take();
    r.s.expect(T::Newline, "")?;

    let mut depth = 0usize;
    let mut state = P::Item;
    loop {
        let t = r.s.kind();
        let diagram = statement(t);
        if state == P::AfterOther || state == P::AfterDiagram {
            if r.s.eat(T::Newline) {
                state = P::ItemOrEnd;
                continue;
            }
            if state == P::AfterOther && diagram.is_some() {
                state = P::Diagram;
                continue;
            }
        } else if let Some(diagram) = diagram {
            if r.diagram(diagram, db)? {
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
            return Err(r.s.fail(
                "'title', 'accDescription', 'acc_title', 'acc_descr', 'acc_descr_multiline_value', 'PERSON', 'REL'",
            ));
        }

        if depth > 0 {
            if !r.s.eat(T::Rbrace) {
                return Err(r.s.fail("'NEWLINE', 'RBRACE'"));
            }
            db.pop_boundary_parse_stack();
            depth -= 1;
            state = P::AfterDiagram;
        } else {
            if !r.s.eat(T::Eof) {
                return Err(r.s.fail("'NEWLINE', 'EOF'"));
            }
            r.s.expect(T::End, "")?;
            db.set_c4_type(header);
            return Ok(());
        }
    }
}
