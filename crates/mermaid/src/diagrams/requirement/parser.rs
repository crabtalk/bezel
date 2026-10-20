//! Pele's `diagrams/requirement/parser.ts`.

use super::{
    db::RequirementModel,
    lexer::tokenize,
    tokens::T,
    types::{Relationship, RequirementType, Risk, VerifyMethod},
};
use crate::{detect::Kind, error::Error, geom::Direction, js, token::Stream};

const REQUIREMENT_TYPES: [(T, RequirementType); 6] = [
    (T::Requirement, RequirementType::Requirement),
    (T::FunctionalRequirement, RequirementType::Functional),
    (T::InterfaceRequirement, RequirementType::Interface),
    (T::PerformanceRequirement, RequirementType::Performance),
    (T::PhysicalRequirement, RequirementType::Physical),
    (T::DesignConstraint, RequirementType::DesignConstraint),
];
const RISK_LEVELS: [(T, Risk); 3] = [
    (T::LowRisk, Risk::Low),
    (T::MedRisk, Risk::Medium),
    (T::HighRisk, Risk::High),
];
const VERIFY_TYPES: [(T, VerifyMethod); 4] = [
    (T::VerifyAnalysis, VerifyMethod::Analysis),
    (T::VerifyDemonstration, VerifyMethod::Demonstration),
    (T::VerifyInspection, VerifyMethod::Inspection),
    (T::VerifyTest, VerifyMethod::Test),
];
const RELATIONSHIPS: [(T, Relationship); 7] = [
    (T::Contains, Relationship::Contains),
    (T::Copies, Relationship::Copies),
    (T::Derives, Relationship::Derives),
    (T::Satisfies, Relationship::Satisfies),
    (T::Verifies, Relationship::Verifies),
    (T::Refines, Relationship::Refines),
    (T::Traces, Relationship::Traces),
];
const STRINGS: [T; 2] = [T::Qstring, T::Unqstring];
const STYLE_PARTS: [T; 5] = [T::Alpha, T::Colon, T::Semicolon, T::Minus, T::Brkt];
const REQUIREMENT_FIELDS: [T; 4] = [T::Id, T::Text, T::Risk, T::Verifymthd];
const ELEMENT_FIELDS: [T; 2] = [T::Type, T::Docref];
const DIRECTIVES: [T; 3] = [T::AccTitle, T::AccDescr, T::AccDescrMultilineValue];
const DIRECTIONS: [(T, Direction); 4] = [
    (T::DirectionTb, Direction::TopToBottom),
    (T::DirectionBt, Direction::BottomToTop),
    (T::DirectionRl, Direction::RightToLeft),
    (T::DirectionLr, Direction::LeftToRight),
];

fn kinds<V>(table: &[(T, V)]) -> impl Iterator<Item = T> + '_ {
    table.iter().map(|&(kind, _)| kind)
}

/// A field of a requirement's or an element's body.
enum Field<'t> {
    Id(&'t str),
    Text(&'t str),
    Risk(Risk),
    VerifyMethod(VerifyMethod),
    Type(&'t str),
    DocRef(&'t str),
}

struct Parser<'t, 's> {
    s: Stream<'t, 's, T>,
}

impl<'t> Parser<'t, '_> {
    /// The value of the current token if it is one of the keywords in
    /// `table`.
    fn keyword<V: Copy>(&mut self, table: &[(T, V)]) -> Result<V, Error> {
        let Some(&(_, value)) = table.iter().find(|&&(kind, _)| self.s.is(kind)) else {
            return Err(self.s.fail_expecting(kinds(table)));
        };
        self.s.bump();
        Ok(value)
    }

    fn string(&mut self) -> Result<&'t str, Error> {
        if !self.s.is_any(&STRINGS) {
            return Err(self.s.fail_expecting(STRINGS));
        }
        Ok(self.s.take())
    }

    fn directive(&mut self, db: &mut RequirementModel) -> Result<bool, Error> {
        if self.s.eat(T::AccTitle) {
            db.titles
                .set_acc_title(js::trim(self.s.expect(T::AccTitleValue, "")?));
        } else if self.s.eat(T::AccDescr) {
            db.titles
                .set_acc_descr(js::trim(self.s.expect(T::AccDescrValue, "")?));
        } else if self.s.is(T::AccDescrMultilineValue) {
            db.titles.set_acc_descr(js::trim(self.s.take()));
        } else {
            return Ok(false);
        }
        Ok(true)
    }

    fn id_list(&mut self) -> Result<Vec<&'t str>, Error> {
        let mut list = Vec::new();
        loop {
            if !self.s.is_any(&STRINGS) && !self.s.is(T::Alpha) {
                return Err(self.s.fail_expecting(STRINGS.into_iter().chain([T::Alpha])));
            }
            list.push(self.s.take());
            if !self.s.eat(T::Comma) {
                return Ok(list);
            }
        }
    }

    fn styles_opt(&mut self) -> Result<Vec<String>, Error> {
        let mut styles = Vec::new();
        loop {
            if !self.s.is_any(&STYLE_PARTS) {
                return Err(self.s.fail_expecting(STYLE_PARTS));
            }
            let mut style = self.s.take().to_string();
            while self.s.is_any(&STYLE_PARTS) {
                style.push_str(self.s.take());
            }
            styles.push(style);
            if !self.s.eat(T::Comma) {
                return Ok(styles);
            }
        }
    }

    fn class_list(&mut self) -> Result<Option<Vec<&'t str>>, Error> {
        let mut classes = None;
        if self.s.eat(T::StyleSeparator) {
            classes = Some(self.id_list()?);
            self.s.expect(T::StructStart, "")?;
        } else if !self.s.eat(T::StructStart) {
            return Err(self.s.fail_expecting([T::StructStart, T::StyleSeparator]));
        }
        self.s.expect(T::Newline, "")?;
        Ok(classes)
    }

    /// Reads a body up to its closing brace. The grammar is right recursive,
    /// so Mermaid applies the fields last to first and the first of two
    /// repeated fields wins.
    fn body(&mut self, db: &mut RequirementModel, requirement: bool) -> Result<(), Error> {
        let fields: &[T] = if requirement {
            &REQUIREMENT_FIELDS
        } else {
            &ELEMENT_FIELDS
        };
        let mut values = Vec::new();
        loop {
            if self.s.eat(T::StructStop) {
                break;
            }
            if self.s.eat(T::Newline) {
                continue;
            }
            let field = self.s.kind();
            if !fields.contains(&field) {
                return Err(self
                    .s
                    .fail_expecting(fields.iter().copied().chain([T::Newline, T::StructStop])));
            }
            self.s.bump();
            self.s.expect(T::Colonsep, "")?;
            values.push(match field {
                T::Id => Field::Id(self.string()?),
                T::Text => Field::Text(self.string()?),
                T::Risk => Field::Risk(self.keyword(&RISK_LEVELS)?),
                T::Verifymthd => Field::VerifyMethod(self.keyword(&VERIFY_TYPES)?),
                T::Type => Field::Type(self.string()?),
                _ => Field::DocRef(self.string()?),
            });
            self.s.expect(T::Newline, "")?;
        }
        for field in values.into_iter().rev() {
            match field {
                Field::Id(id) => db.set_new_req_id(id),
                Field::Text(text) => db.set_new_req_text(text),
                Field::Risk(risk) => db.set_new_req_risk(risk),
                Field::VerifyMethod(method) => db.set_new_req_verify_method(method),
                Field::Type(kind) => db.set_new_element_type(kind),
                Field::DocRef(doc_ref) => db.set_new_element_doc_ref(doc_ref),
            }
        }
        Ok(())
    }
}

pub fn parse_requirement(src: &str, db: &mut RequirementModel) -> Result<(), Error> {
    let tokens = tokenize(src);
    let mut r = Parser {
        s: Stream::new(Kind::Requirement, src, &tokens),
    };

    while r.directive(db)? {
        r.s.eat(T::Newline);
    }
    if !r.s.eat(T::Rd) {
        return Err(r.s.fail_expecting([T::Rd].into_iter().chain(DIRECTIVES)));
    }
    r.s.expect(T::Newline, "")?;

    loop {
        let t = r.s.kind();
        if t == T::Eof {
            return Ok(());
        }
        if r.s.eat(T::Newline) || r.directive(db)? {
            continue;
        }
        if let Some(&(_, direction)) = DIRECTIONS.iter().find(|&&(kind, _)| kind == t) {
            r.s.bump();
            db.direction = direction;
        } else if kinds(&REQUIREMENT_TYPES).any(|kind| kind == t) {
            let kind = r.keyword(&REQUIREMENT_TYPES)?;
            let name = r.string()?;
            let classes = r.class_list()?;
            r.body(db, true)?;
            db.add_requirement(name, kind);
            if let Some(classes) = classes {
                db.set_class(&[name], &classes)?;
            }
        } else if r.s.eat(T::Element) {
            let name = r.string()?;
            let classes = r.class_list()?;
            r.body(db, false)?;
            db.add_element(name);
            if let Some(classes) = classes {
                db.set_class(&[name], &classes)?;
            }
        } else if r.s.eat(T::Style) {
            let ids = r.id_list()?;
            let styles = r.styles_opt()?;
            db.set_css_style(&ids, &styles)?;
        } else if r.s.eat(T::Classdef) {
            let ids = r.id_list()?;
            let styles = r.styles_opt()?;
            db.define_class(&ids, &styles)?;
        } else if r.s.eat(T::Class) {
            let ids = r.id_list()?;
            let names = r.id_list()?;
            db.set_class(&ids, &names)?;
        } else if r.s.is_any(&STRINGS) {
            let id = r.s.take();
            let next = r.s.kind();
            if r.s.eat(T::StyleSeparator) {
                let names = r.id_list()?;
                db.set_class(&[id], &names)?;
            } else if r.s.eat(T::EndArrowL) || r.s.eat(T::Line) {
                let relationship = r.keyword(&RELATIONSHIPS)?;
                r.s.expect(
                    if next == T::Line {
                        T::EndArrowR
                    } else {
                        T::Line
                    },
                    "",
                )?;
                let other = r.string()?;
                if next == T::Line {
                    db.add_relationship(relationship, id, other);
                } else {
                    db.add_relationship(relationship, other, id);
                }
            } else {
                return Err(r
                    .s
                    .fail_expecting([T::StyleSeparator, T::EndArrowL, T::Line]));
            }
        } else {
            return Err(r.s.fail_expecting(
                [T::Newline, T::Eof]
                    .into_iter()
                    .chain(DIRECTIVES)
                    .chain(kinds(&DIRECTIONS))
                    .chain(kinds(&REQUIREMENT_TYPES))
                    .chain([T::Element, T::Classdef, T::Class, T::Style])
                    .chain(STRINGS.into_iter().rev()),
            ));
        }
    }
}
