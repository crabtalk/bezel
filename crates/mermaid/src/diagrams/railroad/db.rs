//! Pele's `diagrams/railroad/db.ts`.

use indexmap::IndexMap;
use serde::{Serialize, Serializer, ser::SerializeMap};

use super::{
    abnf::{abnf_rules, parse_abnf},
    common::Grammar,
    ebnf::{ebnf_rules, parse_ebnf},
    ir::{ir_rules, parse_ir},
    peg::{parse_peg, peg_rules},
    types::{Nodes, Notation, RailroadRule, Rule},
};
use crate::{detect::Kind, error::Error, js::Lazy};

static RE_LEAD: Lazy = Lazy::new(r"^\s+", "g");
static RE_INDENT: Lazy = Lazy::new(r"\n\s+", "g");
static RE_KEYWORD: Lazy = Lazy::new(r"^\s*railroad-(ebnf-|abnf-|peg-)?beta", "i");

#[derive(Clone, Debug, PartialEq)]
pub struct RailroadDb {
    pub notation: Notation,
    pub title: Option<String>,
    pub acc_title: Option<String>,
    pub acc_descr: Option<String>,
    /// Every rule in source order. A name defined twice appears twice.
    pub rules: Vec<RailroadRule>,
    /// What the rules' definitions are made of.
    pub nodes: Nodes,
    /// Each name's place in `rules`.
    by_name: IndexMap<String, usize>,
}

impl Default for RailroadDb {
    fn default() -> Self {
        Self {
            notation: Notation::Railroad,
            title: None,
            acc_title: None,
            acc_descr: None,
            rules: Vec::new(),
            nodes: Nodes::default(),
            by_name: IndexMap::new(),
        }
    }
}

impl RailroadDb {
    pub fn set_title(&mut self, text: String) {
        self.title = Some(text);
    }

    pub fn set_acc_title(&mut self, text: &str) {
        self.acc_title = Some(RE_LEAD.replace(text, |_| String::new()));
    }

    pub fn set_acc_description(&mut self, text: &str) {
        self.acc_descr = Some(RE_INDENT.replace(text, |_| "\n".into()));
    }

    /// A name defined again is drawn again; looking the name up gives the
    /// later definition.
    pub fn add_rule(&mut self, rule: RailroadRule) {
        self.by_name.insert(rule.name.clone(), self.rules.len());
        self.rules.push(rule);
    }

    pub fn get_rule(&self, name: &str) -> Option<&RailroadRule> {
        self.by_name.get(name).map(|&at| &self.rules[at])
    }
}

impl Serialize for RailroadDb {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        struct ByName<'a>(&'a RailroadDb);
        impl Serialize for ByName<'_> {
            fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
                let db = self.0;
                s.collect_map(db.by_name.iter().map(|(name, &at)| {
                    (
                        name,
                        Rule {
                            nodes: &db.nodes,
                            rule: &db.rules[at],
                        },
                    )
                }))
            }
        }
        let mut out = s.serialize_map(None)?;
        out.serialize_entry("type", "railroad")?;
        out.serialize_entry("notation", &self.notation)?;
        if let Some(title) = &self.title {
            out.serialize_entry("title", title)?;
        }
        if let Some(acc_title) = &self.acc_title {
            out.serialize_entry("accTitle", acc_title)?;
        }
        if let Some(acc_descr) = &self.acc_descr {
            out.serialize_entry("accDescr", acc_descr)?;
        }
        let rules: Vec<Rule> = self
            .rules
            .iter()
            .map(|rule| Rule {
                nodes: &self.nodes,
                rule,
            })
            .collect();
        out.serialize_entry("rules", &rules)?;
        out.serialize_entry("byName", &ByName(self))?;
        out.end()
    }
}

/// Mermaid has a detector per keyword, each matching without regard to case.
pub fn notation_of(source: &str) -> Option<Notation> {
    let m = RE_KEYWORD.exec(source)?;
    Some(match m.get(1).map(str::to_lowercase).as_deref() {
        Some("ebnf-") => Notation::Ebnf,
        Some("abnf-") => Notation::Abnf,
        Some("peg-") => Notation::Peg,
        _ => Notation::Railroad,
    })
}

fn grammar(
    source: &str,
    notation: Notation,
    nodes: &mut Nodes,
) -> Result<(Grammar, Vec<RailroadRule>), Error> {
    Ok(match notation {
        Notation::Ebnf => {
            let ast = parse_ebnf(source)?;
            let rules = ebnf_rules(&ast, nodes);
            (ast.grammar, rules)
        }
        Notation::Abnf => {
            let ast = parse_abnf(source)?;
            let rules = abnf_rules(&ast, nodes);
            (ast.grammar, rules)
        }
        Notation::Peg => {
            let ast = parse_peg(source)?;
            let rules = peg_rules(&ast, nodes);
            (ast.grammar, rules)
        }
        Notation::Railroad => {
            let ast = parse_ir(source)?;
            let rules = ir_rules(&ast, nodes);
            (ast.grammar, rules)
        }
    })
}

/// Parses the text in one notation and loads the result into the model, as
/// Mermaid's four parsers do.
pub fn populate(db: &mut RailroadDb, source: &str, notation: Notation) -> Result<(), Error> {
    let (ast, rules) = grammar(source, notation, &mut db.nodes)?;
    db.notation = notation;
    if let Some(text) = ast.acc_descr.filter(|text| !text.is_empty()) {
        db.set_acc_description(&text);
    }
    if let Some(text) = ast.acc_title.filter(|text| !text.is_empty()) {
        db.set_acc_title(&text);
    }
    if let Some(text) = ast.title.filter(|text| !text.is_empty()) {
        db.set_title(text);
    }
    for rule in rules {
        db.add_rule(rule);
    }
    Ok(())
}

pub fn parse_railroad(source: &str, title: Option<&str>) -> Result<RailroadDb, Error> {
    let Some(notation) = notation_of(source) else {
        return Err(Error::syntax("No railroad diagram keyword found.").of(Kind::Railroad));
    };
    let mut db = RailroadDb::default();
    if let Some(title) = title.filter(|title| !title.is_empty()) {
        db.title = Some(title.to_string());
    }
    populate(&mut db, source, notation)?;
    Ok(db)
}
