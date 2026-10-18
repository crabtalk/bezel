//! Pele's `diagrams/treemap/parser.ts`.

use std::sync::LazyLock;

use crate::mermaid::{
    detect::Kind,
    diagrams::common::tokens::{
        Reader, Table, TokenType, acc_descr, acc_descr_value, acc_title, acc_title_value, title,
        title_value, tokenize,
    },
    error::Error,
    js::{self, Lazy},
};

const ACC_DESCR: i32 = 0;
const ACC_TITLE: i32 = 1;
const TITLE: i32 = 2;
const KEYWORD: i32 = 3;
const CLASS_DEF: i32 = 4;
const STYLE_SEPARATOR: i32 = 5;
const SEPARATOR: i32 = 6;
const COMMA: i32 = 7;
const INDENTATION: i32 = 8;
const ID: i32 = 12;
const NUMBER: i32 = 13;
const STRING: i32 = 14;

/// Mermaid's treemap grammar has its own terminals. Every run of spaces or
/// tabs is an INDENTATION token, since that comes before WS, and line breaks
/// are hidden, so rows are told apart by their tokens alone and the
/// indentation of a row is whatever whitespace precedes it.
pub static TREEMAP_TOKENS: LazyLock<Table> = LazyLock::new(|| {
    Table::new(vec![
        acc_descr(),
        acc_title(),
        title(),
        TokenType::regex("TREEMAP_KEYWORD", r"treemap-beta|treemap", None),
        TokenType::regex(
            "CLASS_DEF",
            r"classDef\s+([a-zA-Z_][a-zA-Z0-9_]+)(?:\s+([^;\r\n]*))?(?:;)?",
            None,
        ),
        TokenType::regex("STYLE_SEPARATOR", r":::", None),
        TokenType::regex("SEPARATOR", r":", None),
        TokenType::regex("COMMA", r",", None),
        TokenType::regex("INDENTATION", r"[ \t]{1,}", None),
        TokenType::regex("WS", r"[ \t]+", None).hidden(),
        TokenType::regex("ML_COMMENT", r"\%\%[^\n]*", None).hidden(),
        TokenType::regex("NL", r"\r?\n", None).hidden(),
        TokenType::regex("ID2", r"[a-zA-Z_][a-zA-Z0-9_]*", None),
        TokenType::regex("NUMBER2", r"[0-9_\.\,]+", None),
        TokenType::regex("STRING2", r#""[^"]*"|'[^']*'"#, None),
    ])
});

static RE_CLASS_DEF: Lazy = Lazy::new(r"classDef\s+([A-Z_a-z]\w+)(?:\s+([^\n\r;]*))?;?", "");
static RE_COMMA: Lazy = Lazy::new(r",", "g");

pub enum ItemKind {
    Section,
    Leaf(f64),
}

pub struct ItemAst {
    pub kind: ItemKind,
    pub name: String,
    pub class_selector: Option<String>,
}

pub enum RowAst {
    Row {
        indent: Option<usize>,
        item: ItemAst,
    },
    ClassDef {
        class_name: String,
        style_text: Option<String>,
    },
}

#[derive(Default)]
pub struct TreemapAst {
    pub title: Option<String>,
    pub acc_title: Option<String>,
    pub acc_descr: Option<String>,
    pub rows: Vec<RowAst>,
}

pub fn parse_treemap(src: &str) -> Result<TreemapAst, Error> {
    let tokens = tokenize(src, &TREEMAP_TOKENS, Kind::Treemap)?;
    let mut r = Reader::new(&tokens, &TREEMAP_TOKENS, Kind::Treemap);
    let kinds = &tokens.kinds;
    let mut ast = TreemapAst::default();

    r.expect(KEYWORD)?;
    while r.kind() != -1 {
        let kind = r.kind();
        if kind == ACC_DESCR {
            ast.acc_descr = Some(acc_descr_value(r.take()));
            continue;
        }
        if kind == ACC_TITLE {
            ast.acc_title = Some(acc_title_value(r.take()));
            continue;
        }
        if kind == TITLE {
            ast.title = Some(title_value(r.take()));
            continue;
        }
        if kind != INDENTATION && kind != STRING && kind != CLASS_DEF {
            return Err(r.fail("a node, a class definition, or a title"));
        }

        let indent = (kind == INDENTATION).then(|| js::len(r.take()));
        if r.kind() == CLASS_DEF {
            let text = r.take();
            let m = RE_CLASS_DEF
                .exec(text)
                .expect("a CLASS_DEF token is a class definition");
            ast.rows.push(RowAst::ClassDef {
                class_name: m.str(1).to_string(),
                style_text: m.get(2).filter(|t| !t.is_empty()).map(str::to_string),
            });
            continue;
        }

        let quoted = r.expect(STRING)?;
        let name = quoted[1..quoted.len() - 1].to_string();
        // A leaf is a name followed by a colon or comma, with or without
        // whitespace before it.
        let after = if kinds[r.i] == INDENTATION {
            r.i + 1
        } else {
            r.i
        };
        let kind = if kinds[after] == SEPARATOR || kinds[after] == COMMA {
            r.i = after + 1;
            r.accept(INDENTATION);
            let number = r.expect(NUMBER)?;
            ItemKind::Leaf(js::parse_float(
                &RE_COMMA.replace(number, |_| String::new()),
            ))
        } else {
            ItemKind::Section
        };
        let class_selector = if r.accept(STYLE_SEPARATOR) {
            Some(r.expect(ID)?.to_string())
        } else {
            None
        };
        ast.rows.push(RowAst::Row {
            indent,
            item: ItemAst {
                kind,
                name,
                class_selector,
            },
        });
    }
    Ok(ast)
}
