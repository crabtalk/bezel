//! Pele's `diagrams/railroad/db.ts`.

use crate::diagrams::common::{db::Titles, tokens::Header};
use indexmap::IndexMap;
use serde::{Serialize, Serializer, ser::SerializeMap};

use super::{
    abnf::parse_abnf,
    ebnf::parse_ebnf,
    ir::parse_ir,
    peg::parse_peg,
    types::{Nodes, Notation, RailroadRule, Rule},
};
use crate::{detect::Kind, error::Error, js::Lazy};

static RE_KEYWORD: Lazy = Lazy::new(r"^\s*railroad-(ebnf-|abnf-|peg-)?beta", "i");

#[derive(Clone, Debug, PartialEq)]
pub struct RailroadDb {
    pub notation: Notation,
    pub titles: Titles,
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
            titles: Titles::default(),
            rules: Vec::new(),
            nodes: Nodes::default(),
            by_name: IndexMap::new(),
        }
    }
}

impl RailroadDb {
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
        let titles = &self.titles;
        if let Some(title) = &titles.title {
            out.serialize_entry("title", title)?;
        }
        if let Some(acc_title) = &titles.acc_title {
            out.serialize_entry("accTitle", acc_title)?;
        }
        if let Some(acc_descr) = &titles.acc_descr {
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

impl Notation {
    /// Mermaid has a detector per keyword, each matching without regard to
    /// case.
    pub fn detect(source: &str) -> Option<Self> {
        let m = RE_KEYWORD.exec(source)?;
        Some(match m.get(1).map(str::to_lowercase).as_deref() {
            Some("ebnf-") => Self::Ebnf,
            Some("abnf-") => Self::Abnf,
            Some("peg-") => Self::Peg,
            _ => Self::Railroad,
        })
    }

    /// Parses `source` as written in this notation, its rules' nodes added to
    /// `nodes`.
    fn parse(self, source: &str, nodes: &mut Nodes) -> Result<(Header, Vec<RailroadRule>), Error> {
        Ok(match self {
            Self::Ebnf => parse_ebnf(source)?.into_rules(nodes),
            Self::Abnf => parse_abnf(source)?.into_rules(nodes),
            Self::Peg => parse_peg(source)?.into_rules(nodes),
            Self::Railroad => parse_ir(source)?.into_rules(nodes),
        })
    }
}

impl RailroadDb {
    /// Parses the text in its notation, with `title` from the front matter,
    /// as Mermaid's four parsers do.
    pub fn parse(source: &str, title: Option<&str>) -> Result<Self, Error> {
        let Some(notation) = Notation::detect(source) else {
            return Err(Error::syntax("No railroad diagram keyword found.").of(Kind::Railroad));
        };
        let mut db = Self::default();
        let (header, rules) = notation.parse(source, &mut db.nodes)?;
        db.notation = notation;
        db.titles = header.titles(title.filter(|title| !title.is_empty()));
        for rule in rules {
            db.add_rule(rule);
        }
        Ok(db)
    }
}
