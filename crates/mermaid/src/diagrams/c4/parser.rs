//! Pele's `diagrams/c4/parser.ts`.

use super::{
    db::C4Model,
    lexer::tokenize,
    tokens::T,
    types::{
        C4Attr, C4Named, C4Type, NodeType, RelKind, ShapeBase, ShapeKind,
        Store::{Db, Queue},
    },
};
use crate::{
    detect::Kind,
    error::Error,
    js,
    token::{Stream, after_keyword},
};

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

/// What a diagram statement adds to the model.
#[derive(Clone, Copy)]
enum Statement {
    Shape(ShapeKind),
    /// The type Mermaid inserts as the third attribute, if any.
    Boundary(Option<&'static str>),
    ContainerBoundary,
    DeploymentNode(NodeType),
    Rel(RelKind),
    /// Takes a sequence number first, which Mermaid ignores.
    RelIndex,
    UpdateElStyle,
    UpdateRelStyle,
    UpdateLayoutConfig,
}

impl T {
    fn c4_type(self) -> Option<C4Type> {
        Some(match self {
            T::C4Context => C4Type::Context,
            T::C4Container => C4Type::Container,
            T::C4Component => C4Type::Component,
            T::C4Dynamic => C4Type::Dynamic,
            T::C4Deployment => C4Type::Deployment,
            _ => return None,
        })
    }

    /// A statement that adds nothing to the diagram's elements.
    fn is_other(self) -> bool {
        matches!(
            self,
            T::Title | T::AccDescription | T::AccTitle | T::AccDescr | T::AccDescrMultilineValue
        )
    }

    fn statement(self) -> Option<Statement> {
        use Statement::*;
        let shape = |base, external, store| Shape(ShapeKind::new(base, external, store));
        Some(match self {
            T::Person => shape(ShapeBase::Person, false, None),
            T::PersonExt => shape(ShapeBase::Person, true, None),
            T::System => shape(ShapeBase::System, false, None),
            T::SystemDb => shape(ShapeBase::System, false, Some(Db)),
            T::SystemQueue => shape(ShapeBase::System, false, Some(Queue)),
            T::SystemExt => shape(ShapeBase::System, true, None),
            T::SystemExtDb => shape(ShapeBase::System, true, Some(Db)),
            T::SystemExtQueue => shape(ShapeBase::System, true, Some(Queue)),
            T::Container => shape(ShapeBase::Container, false, None),
            T::ContainerDb => shape(ShapeBase::Container, false, Some(Db)),
            T::ContainerQueue => shape(ShapeBase::Container, false, Some(Queue)),
            T::ContainerExt => shape(ShapeBase::Container, true, None),
            T::ContainerExtDb => shape(ShapeBase::Container, true, Some(Db)),
            T::ContainerExtQueue => shape(ShapeBase::Container, true, Some(Queue)),
            T::Component => shape(ShapeBase::Component, false, None),
            T::ComponentDb => shape(ShapeBase::Component, false, Some(Db)),
            T::ComponentQueue => shape(ShapeBase::Component, false, Some(Queue)),
            T::ComponentExt => shape(ShapeBase::Component, true, None),
            T::ComponentExtDb => shape(ShapeBase::Component, true, Some(Db)),
            T::ComponentExtQueue => shape(ShapeBase::Component, true, Some(Queue)),
            T::EnterpriseBoundary => Boundary(Some("ENTERPRISE")),
            T::SystemBoundary => Boundary(Some("SYSTEM")),
            T::Boundary => Boundary(None),
            T::ContainerBoundary => ContainerBoundary,
            T::Node => DeploymentNode(NodeType::Node),
            T::NodeL => DeploymentNode(NodeType::NodeL),
            T::NodeR => DeploymentNode(NodeType::NodeR),
            T::Rel => Rel(RelKind::Rel),
            T::Birel => Rel(RelKind::Birel),
            T::RelU => Rel(RelKind::RelU),
            T::RelD => Rel(RelKind::RelD),
            T::RelL => Rel(RelKind::RelL),
            T::RelR => Rel(RelKind::RelR),
            T::RelB => Rel(RelKind::RelB),
            T::RelIndex => RelIndex,
            T::UpdateElStyle => UpdateElStyle,
            T::UpdateRelStyle => UpdateRelStyle,
            T::UpdateLayoutConfig => UpdateLayoutConfig,
            _ => return None,
        })
    }
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
            T::Title => db.titles.set_title(after_keyword(self.s.take(), 5)),
            T::AccDescription => db.titles.set_acc_descr(after_keyword(self.s.take(), 14)),
            T::AccDescrMultilineValue => db.titles.set_acc_descr(js::trim(self.s.take())),
            T::AccTitle => {
                self.s.bump();
                if !self.s.is(T::AccTitleValue) {
                    return Err(self.s.fail("'acc_title_value'"));
                }
                // The grammar hands `accTitle:` to setTitle, so it becomes the
                // visible title.
                db.titles.set_title(js::trim(self.s.take()));
            }
            _ => {
                self.s.bump();
                if !self.s.is(T::AccDescrValue) {
                    return Err(self.s.fail("'acc_descr_value'"));
                }
                db.titles.set_acc_descr(js::trim(self.s.take()));
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
            Statement::Shape(kind) => db.add_shape(kind, &attrs),
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
                db.add_rel(RelKind::Rel, &attrs);
            }
            Statement::UpdateElStyle => db.update_el_style(&attrs),
            Statement::UpdateRelStyle => db.update_rel_style(&attrs),
            Statement::UpdateLayoutConfig => db.update_layout_config(&attrs),
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

    let Some(header) = r.s.kind().c4_type() else {
        return Err(r.s.fail(HEADERS));
    };
    r.s.bump();
    r.s.expect(T::Newline, "")?;

    let mut depth = 0usize;
    let mut state = P::Item;
    loop {
        let t = r.s.kind();
        let diagram = t.statement();
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
        } else if t.is_other() && state != P::Diagram {
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
            db.c4_type = Some(header);
            return Ok(());
        }
    }
}
