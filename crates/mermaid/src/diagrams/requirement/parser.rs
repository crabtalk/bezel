//! Pele's `diagrams/requirement/parser.ts`.

use super::{
    db::{RequirementModel, relationships, requirement_type, risk_level, verify_type},
    lexer::{Tokens, tokenize},
    tokens::*,
};
use crate::{detect::Kind, error::Error, js};

/// What the keyword tokens stand for, from REQUIREMENT to TRACES in token
/// order.
const VALUES: [&str; 21] = [
    requirement_type::REQUIREMENT,
    requirement_type::FUNCTIONAL_REQUIREMENT,
    requirement_type::INTERFACE_REQUIREMENT,
    requirement_type::PERFORMANCE_REQUIREMENT,
    requirement_type::PHYSICAL_REQUIREMENT,
    requirement_type::DESIGN_CONSTRAINT,
    risk_level::LOW_RISK,
    risk_level::MED_RISK,
    risk_level::HIGH_RISK,
    verify_type::VERIFY_ANALYSIS,
    verify_type::VERIFY_DEMONSTRATION,
    verify_type::VERIFY_INSPECTION,
    verify_type::VERIFY_TEST,
    "",
    relationships::CONTAINS,
    relationships::COPIES,
    relationships::DERIVES,
    relationships::SATISFIES,
    relationships::VERIFIES,
    relationships::REFINES,
    relationships::TRACES,
];

fn is_style_part(t: u8) -> bool {
    matches!(t, ALPHA | COLON | BRKT | MINUS | SEMICOLON)
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

    /// Fails with Jison's wording, naming the token types `from` to `to` and
    /// any others as expected.
    fn fail(&self, from: u8, to: u8, others: &[u8]) -> Error {
        let t = self.t();
        let start = self.tokens.starts[self.i];
        if t == ERROR {
            return Error::at(Kind::Requirement, self.src, start, "", true);
        }
        let expected = (from..=to)
            .chain(others.iter().copied())
            .map(|kind| format!("'{}'", TOKEN_NAMES[kind as usize]))
            .collect::<Vec<_>>()
            .join(", ");
        Error::at(
            Kind::Requirement,
            self.src,
            start,
            &format!("Expecting {expected}, got '{}'", TOKEN_NAMES[t as usize]),
            false,
        )
    }

    fn expect(&mut self, kind: u8) -> Result<&'s str, Error> {
        if self.t() != kind {
            return Err(self.fail(kind, kind, &[]));
        }
        self.i += 1;
        Ok(self.tokens.texts[self.i - 1])
    }

    /// The value of the next token if it is a keyword in the given range of
    /// token types.
    fn keyword(&mut self, from: u8, to: u8) -> Result<&'static str, Error> {
        let t = self.t();
        if t < from || t > to {
            return Err(self.fail(from, to, &[]));
        }
        self.i += 1;
        Ok(VALUES[(t - REQUIREMENT) as usize])
    }

    fn string(&mut self) -> Result<&'s str, Error> {
        let t = self.t();
        if t != UNQSTRING && t != QSTRING {
            return Err(self.fail(QSTRING, UNQSTRING, &[]));
        }
        self.i += 1;
        Ok(self.tokens.texts[self.i - 1])
    }

    fn directive(&mut self, db: &mut RequirementModel) -> Result<bool, Error> {
        let t = self.t();
        if t == ACC_TITLE {
            self.i += 1;
            db.set_acc_title(js::trim(self.expect(ACC_TITLE_VALUE)?));
        } else if t == ACC_DESCR {
            self.i += 1;
            db.set_acc_description(js::trim(self.expect(ACC_DESCR_VALUE)?));
        } else if t == ACC_DESCR_MULTILINE_VALUE {
            db.set_acc_description(js::trim(self.text()));
            self.i += 1;
        } else {
            return Ok(false);
        }
        Ok(true)
    }

    fn id_list(&mut self) -> Result<Vec<&'s str>, Error> {
        let mut list = Vec::new();
        loop {
            let t = self.t();
            if t != ALPHA && t != UNQSTRING && t != QSTRING {
                return Err(self.fail(QSTRING, UNQSTRING, &[ALPHA]));
            }
            list.push(self.text());
            self.i += 1;
            if self.t() != COMMA {
                return Ok(list);
            }
            self.i += 1;
        }
    }

    fn styles_opt(&mut self) -> Result<Vec<String>, Error> {
        let mut styles = Vec::new();
        loop {
            if !is_style_part(self.t()) {
                return Err(self.fail(ALPHA, SEMICOLON, &[MINUS, BRKT]));
            }
            let mut style = self.text().to_string();
            self.i += 1;
            while is_style_part(self.t()) {
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

    fn class_list(&mut self) -> Result<Option<Vec<&'s str>>, Error> {
        let mut classes = None;
        if self.t() == STYLE_SEPARATOR {
            self.i += 1;
            classes = Some(self.id_list()?);
            self.expect(STRUCT_START)?;
        } else if self.t() == STRUCT_START {
            self.i += 1;
        } else {
            return Err(self.fail(STRUCT_START, STRUCT_START, &[STYLE_SEPARATOR]));
        }
        self.expect(NEWLINE)?;
        Ok(classes)
    }

    /// Reads a body up to its closing brace. The grammar is right recursive,
    /// so Mermaid applies the fields last to first and the first of two
    /// repeated fields wins.
    fn body(&mut self, db: &mut RequirementModel, requirement: bool) -> Result<(), Error> {
        let mut fields: Vec<u8> = Vec::new();
        let mut values: Vec<&str> = Vec::new();
        loop {
            let t = self.t();
            if t == STRUCT_STOP {
                self.i += 1;
                break;
            }
            if t == NEWLINE {
                self.i += 1;
                continue;
            }
            let wrong = if requirement {
                !(ID..=VERIFYMTHD).contains(&t)
            } else {
                t != TYPE && t != DOCREF
            };
            if wrong {
                return Err(if requirement {
                    self.fail(ID, VERIFYMTHD, &[NEWLINE, STRUCT_STOP])
                } else {
                    self.fail(TYPE, DOCREF, &[NEWLINE, STRUCT_STOP])
                });
            }
            self.i += 1;
            self.expect(COLONSEP)?;
            values.push(if t == RISK {
                self.keyword(LOW_RISK, HIGH_RISK)?
            } else if t == VERIFYMTHD {
                self.keyword(VERIFY_ANALYSIS, VERIFY_TEST)?
            } else {
                self.string()?
            });
            fields.push(t);
            self.expect(NEWLINE)?;
        }
        for (&field, &value) in fields.iter().zip(&values).rev() {
            match field {
                ID => db.set_new_req_id(value),
                TEXT => db.set_new_req_text(value),
                RISK => db.set_new_req_risk(value),
                VERIFYMTHD => db.set_new_req_verify_method(value),
                TYPE => db.set_new_element_type(value),
                _ => db.set_new_element_doc_ref(value),
            }
        }
        Ok(())
    }
}

pub fn parse_requirement(src: &str, db: &mut RequirementModel) -> Result<(), Error> {
    let tokens = tokenize(src);
    let mut r = Parser {
        tokens: &tokens,
        src,
        i: 0,
    };

    while r.directive(db)? {
        if r.t() == NEWLINE {
            r.i += 1;
        }
    }
    if r.t() != RD {
        return Err(r.fail(RD, RD, &[ACC_TITLE, ACC_DESCR, ACC_DESCR_MULTILINE_VALUE]));
    }
    r.i += 1;
    r.expect(NEWLINE)?;

    loop {
        let t = r.t();
        if t == EOF {
            return Ok(());
        }
        if t == NEWLINE {
            r.i += 1;
        } else if r.directive(db)? {
            continue;
        } else if (DIRECTION_TB..=DIRECTION_LR).contains(&t) {
            r.i += 1;
            db.set_direction(match t {
                DIRECTION_TB => "TB",
                DIRECTION_BT => "BT",
                DIRECTION_RL => "RL",
                _ => "LR",
            });
        } else if (REQUIREMENT..=DESIGN_CONSTRAINT).contains(&t) {
            let kind = r.keyword(REQUIREMENT, DESIGN_CONSTRAINT)?;
            let name = r.string()?;
            let classes = r.class_list()?;
            r.body(db, true)?;
            db.add_requirement(name, kind);
            if let Some(classes) = classes {
                db.set_class(&[name], &classes)?;
            }
        } else if t == ELEMENT {
            r.i += 1;
            let name = r.string()?;
            let classes = r.class_list()?;
            r.body(db, false)?;
            db.add_element(name);
            if let Some(classes) = classes {
                db.set_class(&[name], &classes)?;
            }
        } else if t == STYLE {
            r.i += 1;
            let ids = r.id_list()?;
            let styles = r.styles_opt()?;
            db.set_css_style(&ids, &styles)?;
        } else if t == CLASSDEF {
            r.i += 1;
            let ids = r.id_list()?;
            let styles = r.styles_opt()?;
            db.define_class(&ids, &styles)?;
        } else if t == CLASS {
            r.i += 1;
            let ids = r.id_list()?;
            let names = r.id_list()?;
            db.set_class(&ids, &names)?;
        } else if t == UNQSTRING || t == QSTRING {
            let id = r.text();
            r.i += 1;
            let next = r.t();
            if next == STYLE_SEPARATOR {
                r.i += 1;
                let names = r.id_list()?;
                db.set_class(&[id], &names)?;
            } else if next == END_ARROW_L || next == LINE {
                r.i += 1;
                let relationship = r.keyword(CONTAINS, TRACES)?;
                r.expect(if next == LINE { END_ARROW_R } else { LINE })?;
                let other = r.string()?;
                if next == LINE {
                    db.add_relationship(relationship, id, other);
                } else {
                    db.add_relationship(relationship, other, id);
                }
            } else {
                return Err(r.fail(STYLE_SEPARATOR, STYLE_SEPARATOR, &[END_ARROW_L, LINE]));
            }
        } else {
            return Err(r.fail(
                NEWLINE,
                EOF,
                &[
                    ACC_TITLE,
                    ACC_DESCR,
                    ACC_DESCR_MULTILINE_VALUE,
                    DIRECTION_TB,
                    DIRECTION_BT,
                    DIRECTION_RL,
                    DIRECTION_LR,
                    REQUIREMENT,
                    FUNCTIONAL_REQUIREMENT,
                    INTERFACE_REQUIREMENT,
                    PERFORMANCE_REQUIREMENT,
                    PHYSICAL_REQUIREMENT,
                    DESIGN_CONSTRAINT,
                    ELEMENT,
                    CLASSDEF,
                    CLASS,
                    STYLE,
                    UNQSTRING,
                    QSTRING,
                ],
            ));
        }
    }
}
