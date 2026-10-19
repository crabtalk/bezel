//! Pele's `diagrams/treeview/parser.ts`.

use std::sync::LazyLock;

use crate::{
    detect::Kind,
    diagrams::common::{
        acc_descr::acc_descr,
        tokens::{
            Reader, Table, TokenType, acc_descr_value, acc_title, acc_title_value, keyword, title,
            title_value, tokenize,
        },
    },
    error::Error,
    js,
    util::chars::{code_at, is_blank, width_at},
};

const KEYWORD: i32 = 0;
const ACC_DESCR: i32 = 1;
const ACC_TITLE: i32 = 2;
const TITLE: i32 = 3;
const CLASS_ANNOTATION: i32 = 4;
const ICON_ANNOTATION: i32 = 5;
const DESC_ANNOTATION: i32 = 6;
const INDENTATION: i32 = 7;
const QUOTED_NAME: i32 = 8;
const BARE_NAME: i32 = 12;

static TOKENS: LazyLock<Table> = LazyLock::new(|| {
    Table::new(vec![
        keyword("treeView-beta").longer(&["BARE_NAME"]),
        acc_descr("ACC_DESCR"),
        acc_title(),
        title(),
        TokenType::regex(
            "CLASS_ANNOTATION",
            r"[ \t]+:::[ \t]*[A-Za-z_][\w-]*",
            None,
        ),
        TokenType::regex(
            "ICON_ANNOTATION",
            r"[ \t]+icon\([\w-]*(?::[\w-]+)?\)",
            None,
        ),
        TokenType::regex("DESC_ANNOTATION", r"[ \t]+##[^\n\r]*", None),
        TokenType::regex("INDENTATION", r"[ \t]{1,}", None),
        TokenType::regex("QUOTED_NAME", r#""[^"]*"|'[^']*'"#, None),
        TokenType::regex("WS", r"[ \t]+", None).hidden(),
        TokenType::regex("ML_COMMENT", r"\%\%[^\n]*", None).hidden(),
        TokenType::regex("NL", r"\r?\n", None).hidden(),
        TokenType::regex(
            "BARE_NAME",
            r#"(?!:::|icon\(|##)[^ \t\n\r"'](?:(?![ \t]+:::[ \t]*[A-Za-z_]|[ \t]+icon\(|[ \t]+##)[^\n\r])*"#,
            None,
        )
        .matcher(match_bare_name),
    ])
});

pub struct TreeNodeAst {
    pub indent: Option<usize>,
    pub name: String,
    pub class_annotation: Option<String>,
    pub icon_annotation: Option<String>,
    pub desc_annotation: Option<String>,
}

#[derive(Default)]
pub struct TreeViewAst {
    pub title: Option<String>,
    pub acc_title: Option<String>,
    pub acc_descr: Option<String>,
    pub nodes: Vec<TreeNodeAst>,
}

fn annotation_at(src: &str, at: usize) -> bool {
    let rest = src.get(at..).unwrap_or("");
    if rest.starts_with("##") || rest.starts_with("icon(") {
        return true;
    }
    if !rest.starts_with(":::") {
        return false;
    }
    let mut i = at + 3;
    while is_blank(code_at(src, i)) {
        i += 1;
    }
    let c = code_at(src, i);
    (65..=90).contains(&c) || (97..=122).contains(&c) || c == 95
}

/// The pattern looks ahead from every blank for an annotation, which rescans
/// long runs of blanks. This looks once per run.
fn match_bare_name(src: &str, at: usize) -> Option<usize> {
    let first = code_at(src, at);
    if is_blank(first) || first == 10 || first == 13 || first == 34 || first == 39 {
        return None;
    }
    let rest = &src[at..];
    if rest.starts_with(":::") || rest.starts_with("icon(") || rest.starts_with("##") {
        return None;
    }
    let n = src.len();
    let mut i = at + width_at(src, at);
    while i < n {
        let c = code_at(src, i);
        if c == 10 || c == 13 {
            break;
        }
        if is_blank(c) {
            let mut j = i + 1;
            while is_blank(code_at(src, j)) {
                j += 1;
            }
            if annotation_at(src, j) {
                break;
            }
            i = j;
        } else {
            i += width_at(src, i);
        }
    }
    Some(i)
}

fn trim_blanks_end(text: &str) -> &str {
    text.trim_end_matches([' ', '\t'])
}

pub fn parse_tree_view(src: &str) -> Result<TreeViewAst, Error> {
    let tokens = tokenize(src, &TOKENS, Kind::TreeView)?;
    let mut r = Reader::new(&tokens, &TOKENS, Kind::TreeView);
    let mut ast = TreeViewAst::default();
    r.expect(KEYWORD)?;
    loop {
        match r.kind() {
            ACC_DESCR => ast.acc_descr = Some(acc_descr_value(r.take())),
            ACC_TITLE => ast.acc_title = Some(acc_title_value(r.take())),
            TITLE => ast.title = Some(title_value(r.take())),
            _ => break,
        }
    }
    while r.kind() != -1 {
        let mut node = TreeNodeAst {
            indent: None,
            name: String::new(),
            class_annotation: None,
            icon_annotation: None,
            desc_annotation: None,
        };
        if r.kind() == INDENTATION {
            node.indent = Some(js::len(r.take()));
        }
        if r.kind() == QUOTED_NAME {
            let text = r.take();
            node.name = text[1..text.len() - 1].to_string();
        } else if r.kind() == BARE_NAME {
            node.name = trim_blanks_end(r.take()).to_string();
        } else {
            return Err(r.fail("a name"));
        }
        loop {
            match r.kind() {
                CLASS_ANNOTATION => {
                    node.class_annotation = Some(js::trim(&js::trim(r.take())[3..]).to_string());
                }
                ICON_ANNOTATION => {
                    let trimmed = js::trim(r.take());
                    node.icon_annotation = Some(trimmed[5..trimmed.len() - 1].to_string());
                }
                DESC_ANNOTATION => {
                    node.desc_annotation = Some(js::trim(&js::trim(r.take())[2..]).to_string());
                }
                _ => break,
            }
        }
        ast.nodes.push(node);
    }
    Ok(ast)
}
