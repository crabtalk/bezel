//! Pele's `diagrams/treemap/parser.ts`.

use crate::util::chars::inner;

use crate::{
    detect::Kind,
    diagrams::common::tokens::{
        Header, Reader, TokenType, acc_descr, acc_title, title, token_kinds,
    },
    error::Error,
    js::{self, Lazy},
};

token_kinds! {
    /// Mermaid's treemap grammar has its own terminals. Every run of spaces
    /// or tabs is an INDENTATION token, since that comes before WS, and line
    /// breaks are hidden, so rows are told apart by their tokens alone and
    /// the indentation of a row is whatever whitespace precedes it.
    enum Token {
        AccDescr = acc_descr(),
        AccTitle = acc_title(),
        Title = title(),
        Keyword = TokenType::regex("TREEMAP_KEYWORD", r"treemap-beta|treemap", None),
        ClassDef = TokenType::regex(
            "CLASS_DEF",
            r"classDef\s+([a-zA-Z_][a-zA-Z0-9_]+)(?:\s+([^;\r\n]*))?(?:;)?",
            None,
        ),
        StyleSeparator = TokenType::regex("STYLE_SEPARATOR", r":::", None),
        Separator = TokenType::regex("SEPARATOR", r":", None),
        Comma = TokenType::regex("COMMA", r",", None),
        Indentation = TokenType::regex("INDENTATION", r"[ \t]{1,}", None),
        Ws = TokenType::regex("WS", r"[ \t]+", None).hidden(),
        Comment = TokenType::regex("ML_COMMENT", r"\%\%[^\n]*", None).hidden(),
        Newline = TokenType::regex("NL", r"\r?\n", None).hidden(),
        Id = TokenType::regex("ID2", r"[a-zA-Z_][a-zA-Z0-9_]*", None),
        Number = TokenType::regex("NUMBER2", r"[0-9_\.\,]+", None),
        String = TokenType::regex("STRING2", r#""[^"]*"|'[^']*'"#, None),
    }
}

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
    pub header: Header,
    pub rows: Vec<RowAst>,
}

pub fn parse_treemap(src: &str) -> Result<TreemapAst, Error> {
    let mut r = Reader::<Token>::new(src, Kind::Treemap)?;
    let mut ast = TreemapAst::default();

    r.expect(Token::Keyword)?;
    while let Some(kind) = r.kind() {
        if r.heading(&mut ast.header) {
            continue;
        }
        if !matches!(kind, Token::Indentation | Token::String | Token::ClassDef) {
            return Err(r.fail("a node, a class definition, or a title"));
        }

        let indent = (kind == Token::Indentation).then(|| r.take().len());
        if r.at(Token::ClassDef) {
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

        let name = inner(r.expect(Token::String)?).to_string();
        // A leaf is a name followed by a colon or comma, with or without
        // whitespace before it.
        let after = usize::from(r.at(Token::Indentation));
        let kind = if matches!(r.peek(after), Some(Token::Separator | Token::Comma)) {
            for _ in 0..=after {
                r.bump();
            }
            r.accept(Token::Indentation);
            let number = r.expect(Token::Number)?;
            ItemKind::Leaf(js::parse_float(
                &RE_COMMA.replace(number, |_| String::new()),
            ))
        } else {
            ItemKind::Section
        };
        let class_selector = if r.accept(Token::StyleSeparator) {
            Some(r.expect(Token::Id)?.to_string())
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
