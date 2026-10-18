//! Pele's `diagrams/radar/parser.ts`.

use std::sync::LazyLock;

use crate::mermaid::{
    detect::Kind,
    diagrams::common::tokens::{
        Reader, Scanner, Table, TokenType, Tokens, acc_descr, acc_descr_value, acc_title,
        acc_title_value, directive, keyword, newline, single_line_comment, string, string_value,
        title, title_value, tokenize, whitespace, yaml,
    },
    error::Error,
    js::{self, Lazy},
    util::chars::code_at,
};

const RADAR_COLON: i32 = 0;
const RADAR: i32 = 1;
const SHOW_LEGEND: i32 = 2;
const GRATICULE: i32 = 3;
const CURVE: i32 = 4;
const TICKS: i32 = 5;
const AXIS: i32 = 6;
const MAX: i32 = 7;
const MIN: i32 = 8;
const COLON: i32 = 9;
const COMMA: i32 = 10;
const OPEN: i32 = 11;
const CLOSE: i32 = 12;
const OPEN_BRACE: i32 = 13;
const CLOSE_BRACE: i32 = 14;
const GRATICULE_VALUE: i32 = 15;
const BOOLEAN: i32 = 16;
const ACC_DESCR: i32 = 17;
const ACC_TITLE: i32 = 18;
const TITLE: i32 = 19;
const NUMBER: i32 = 20;
const STRING: i32 = 21;
const ID: i32 = 22;
const NEWLINE: i32 = 23;

static RE_ACC_DESCR: Lazy = Lazy::new(
    r"[\t ]*accDescr(?:[\t ]*:([^\n\r]*?(?=%%)|[^\n\r]*)|\s*\{([^}]*)\})",
    "y",
);
static RE_BLOCK: Lazy = Lazy::new(r"[\t ]*accDescr\s*{", "y");

pub static RADAR_TOKENS: LazyLock<Table> = LazyLock::new(|| {
    let word = |text: &'static str| TokenType::literal(text, text).longer(&["ID"]);
    let mark = |text: &'static str| TokenType::literal(text, text);
    let mut accessible = acc_descr();
    accessible.scanner = Some(acc_descr_scanner);
    Table::new(vec![
        mark("radar-beta:"),
        keyword("radar-beta").longer(&["ID"]),
        word("showLegend"),
        word("graticule"),
        word("curve"),
        word("ticks"),
        word("axis"),
        word("max"),
        word("min"),
        mark(":"),
        mark(","),
        mark("["),
        mark("]"),
        mark("{"),
        mark("}"),
        TokenType::regex("GRATICULE", r"circle|polygon", None),
        TokenType::regex("BOOLEAN", r"true|false", None),
        accessible,
        acc_title(),
        title(),
        TokenType::regex(
            "NUMBER",
            r"(?:[0-9]+\.[0-9]+(?!\.))|(?:0|[1-9][0-9]*(?!\.))",
            None,
        )
        .scanner(number_scanner),
        string(),
        TokenType::regex("ID", r"[\w]([-\w]*\w)?", None),
        newline(),
        whitespace(),
        yaml(),
        directive(),
        single_line_comment(),
    ])
});

fn digits(src: &str, at: usize) -> usize {
    let mut end = at;
    while (48..=57).contains(&code_at(src, end)) {
        end += 1;
    }
    end
}

/// The NUMBER pattern without its backtracking. As a regular expression it
/// reads to the end of a run of digits from every position in the run, which
/// is quadratic for a long run of zeros.
fn number_scanner() -> Scanner {
    let mut run_from = 0;
    let mut run_end = 0;
    Box::new(move |src, at| {
        if at < run_from || at >= run_end {
            run_from = at;
            run_end = digits(src, at);
        }
        let whole = run_end;
        if whole == at {
            return None;
        }
        let dot = code_at(src, whole) == 46;
        if dot {
            let fraction = digits(src, whole + 1);
            let length = fraction - whole - 1;
            if length > 0 && code_at(src, fraction) != 46 {
                return Some(fraction);
            }
            if length > 1 {
                return Some(fraction - 1);
            }
        }
        if code_at(src, at) == 48 {
            return Some(at + 1);
        }
        if !dot {
            return Some(whole);
        }
        if whole - 1 > at {
            Some(whole - 1)
        } else {
            None
        }
    })
}

/// In the other grammars an `accDescr {` that never closes ends the lexing.
/// Here `accDescr` is also a name and `{` a token, so each unclosed one would
/// read to the end of the text again.
fn acc_descr_scanner() -> Scanner {
    let mut last_brace: Option<Option<usize>> = None;
    Box::new(move |src, at| {
        if let Some(end) = RE_BLOCK.match_end(src, at) {
            let brace = *last_brace.get_or_insert_with(|| src.rfind('}'));
            if brace.is_none_or(|brace| end > brace) {
                return None;
            }
        }
        RE_ACC_DESCR.match_end(src, at)
    })
}

pub struct AxisAst {
    pub name: String,
    pub label: Option<String>,
}

pub struct EntryAst {
    /// Langium leaves the reference unresolved; Mermaid reads only its text.
    pub axis: Option<String>,
    pub value: f64,
}

pub struct CurveAst {
    pub name: String,
    pub label: Option<String>,
    pub entries: Vec<EntryAst>,
}

pub enum OptionValue {
    Bool(bool),
    Number(f64),
    Text(String),
}

pub struct OptionAst {
    pub name: String,
    pub value: OptionValue,
}

#[derive(Default)]
pub struct RadarAst {
    pub title: Option<String>,
    pub acc_title: Option<String>,
    pub acc_descr: Option<String>,
    pub axes: Vec<AxisAst>,
    pub curves: Vec<CurveAst>,
    pub options: Vec<OptionAst>,
}

pub fn lex_radar(src: &str) -> Result<Tokens<'_>, Error> {
    tokenize(src, &RADAR_TOKENS, Kind::Radar)
}

fn newlines(r: &mut Reader) {
    while r.kind() == NEWLINE {
        r.i += 1;
    }
}

fn end_of_line(r: &mut Reader) -> Result<(), Error> {
    if r.kind() == -1 {
        return Ok(());
    }
    if r.kind() != NEWLINE {
        return Err(r.fail("a line break or the end of input"));
    }
    newlines(r);
    Ok(())
}

fn label(r: &mut Reader) -> Result<Option<String>, Error> {
    if !r.accept(OPEN) {
        return Ok(None);
    }
    let text = string_value(r.expect(STRING)?);
    r.expect(CLOSE)?;
    Ok(Some(text))
}

fn entry(r: &mut Reader, detailed: bool) -> Result<EntryAst, Error> {
    if !detailed {
        return Ok(EntryAst {
            axis: None,
            value: js::number(r.expect(NUMBER)?),
        });
    }
    let axis = r.expect(ID)?.to_string();
    r.accept(COLON);
    Ok(EntryAst {
        axis: Some(axis),
        value: js::number(r.expect(NUMBER)?),
    })
}

fn curve(r: &mut Reader) -> Result<CurveAst, Error> {
    let name = r.expect(ID)?.to_string();
    let label = label(r)?;
    let mut node = CurveAst {
        name,
        label,
        entries: Vec::new(),
    };
    r.expect(OPEN_BRACE)?;
    newlines(r);
    if r.kind() != NUMBER && r.kind() != ID {
        return Err(r.fail("a number or an axis name"));
    }
    let detailed = r.kind() == ID;
    node.entries.push(entry(r, detailed)?);
    while r.accept(COMMA) {
        newlines(r);
        node.entries.push(entry(r, detailed)?);
    }
    newlines(r);
    r.expect(CLOSE_BRACE)?;
    Ok(node)
}

fn axis(r: &mut Reader) -> Result<AxisAst, Error> {
    let name = r.expect(ID)?.to_string();
    let label = label(r)?;
    Ok(AxisAst { name, label })
}

fn option(r: &mut Reader) -> Result<OptionAst, Error> {
    let kind = r.kind();
    let name = r.take().to_string();
    let value = if kind == SHOW_LEGEND {
        OptionValue::Bool(r.expect(BOOLEAN)? == "true")
    } else if kind == GRATICULE {
        OptionValue::Text(r.expect(GRATICULE_VALUE)?.to_string())
    } else {
        OptionValue::Number(js::number(r.expect(NUMBER)?))
    };
    Ok(OptionAst { name, value })
}

fn is_option(r: &Reader) -> bool {
    matches!(r.kind(), SHOW_LEGEND | TICKS | MAX | MIN | GRATICULE)
}

pub fn parse_radar(src: &str) -> Result<RadarAst, Error> {
    let tokens = lex_radar(src)?;
    let mut r = Reader::new(&tokens, &RADAR_TOKENS, Kind::Radar);
    let r = &mut r;
    let mut ast = RadarAst::default();

    newlines(r);
    if !r.accept(RADAR_COLON) {
        r.expect(RADAR)?;
        r.accept(COLON);
    }
    newlines(r);
    while r.kind() != -1 {
        match r.kind() {
            NEWLINE => r.i += 1,
            ACC_DESCR => {
                ast.acc_descr = Some(acc_descr_value(r.take()));
                end_of_line(r)?;
            }
            ACC_TITLE => {
                ast.acc_title = Some(acc_title_value(r.take()));
                end_of_line(r)?;
            }
            TITLE => {
                ast.title = Some(title_value(r.take()));
                end_of_line(r)?;
            }
            AXIS => {
                r.i += 1;
                loop {
                    ast.axes.push(axis(r)?);
                    if !r.accept(COMMA) {
                        break;
                    }
                }
            }
            CURVE => {
                r.i += 1;
                loop {
                    ast.curves.push(curve(r)?);
                    if !r.accept(COMMA) {
                        break;
                    }
                }
            }
            _ => {
                if !is_option(r) {
                    return Err(r.fail("an axis, a curve, an option, a title, or a line break"));
                }
                ast.options.push(option(r)?);
                while r.accept(COMMA) {
                    if !is_option(r) {
                        return Err(r.fail("an option"));
                    }
                    ast.options.push(option(r)?);
                }
            }
        }
    }
    Ok(ast)
}
