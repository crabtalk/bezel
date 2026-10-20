//! Pele's `diagrams/treeview/parser.ts`.

use crate::{
    detect::Kind,
    diagrams::common::{
        acc_descr::acc_descr,
        tokens::{Header, Reader, TokenType, acc_title, keyword, title, token_kinds},
    },
    error::Error,
    js,
    util::chars::{char_at, inner},
};

token_kinds! {
    enum Token {
        Keyword = keyword("treeView-beta").longer(&[Token::BareName]),
        AccDescr = acc_descr("ACC_DESCR"),
        AccTitle = acc_title(),
        Title = title(),
        ClassAnnotation = TokenType::regex(
            "CLASS_ANNOTATION",
            r"[ \t]+:::[ \t]*[A-Za-z_][\w-]*",
            None,
        ),
        IconAnnotation = TokenType::regex(
            "ICON_ANNOTATION",
            r"[ \t]+icon\([\w-]*(?::[\w-]+)?\)",
            None,
        ),
        DescAnnotation = TokenType::regex("DESC_ANNOTATION", r"[ \t]+##[^\n\r]*", None),
        Indentation = TokenType::regex("INDENTATION", r"[ \t]{1,}", None),
        QuotedName = TokenType::regex("QUOTED_NAME", r#""[^"]*"|'[^']*'"#, None),
        Ws = TokenType::regex("WS", r"[ \t]+", None).hidden(),
        Comment = TokenType::regex("ML_COMMENT", r"\%\%[^\n]*", None).hidden(),
        Newline = TokenType::regex("NL", r"\r?\n", None).hidden(),
        BareName = TokenType::regex(
            "BARE_NAME",
            r#"(?!:::|icon\(|##)[^ \t\n\r"'](?:(?![ \t]+:::[ \t]*[A-Za-z_]|[ \t]+icon\(|[ \t]+##)[^\n\r])*"#,
            None,
        )
        .matcher(match_bare_name),
    }
}

pub struct TreeNodeAst {
    pub indent: Option<usize>,
    pub name: String,
    pub class_annotation: Option<String>,
    pub icon_annotation: Option<String>,
    pub desc_annotation: Option<String>,
}

#[derive(Default)]
pub struct TreeViewAst {
    pub header: Header,
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
    let name = src[at + 3..].trim_start_matches([' ', '\t']);
    name.starts_with(|c: char| c.is_ascii_alphabetic() || c == '_')
}

/// The pattern looks ahead from every blank for an annotation, which rescans
/// long runs of blanks. This looks once per run.
fn match_bare_name(src: &str, at: usize) -> Option<usize> {
    let rest = &src[at..];
    let first = rest.chars().next()?;
    if matches!(first, ' ' | '\t' | '\n' | '\r' | '"' | '\'')
        || rest.starts_with(":::")
        || rest.starts_with("icon(")
        || rest.starts_with("##")
    {
        return None;
    }
    let mut i = at + first.len_utf8();
    while let Some(c) = char_at(src, i) {
        if matches!(c, '\n' | '\r') {
            break;
        }
        if matches!(c, ' ' | '\t') {
            let j = src.len() - src[i..].trim_start_matches([' ', '\t']).len();
            if annotation_at(src, j) {
                break;
            }
            i = j;
        } else {
            i += c.len_utf8();
        }
    }
    Some(i)
}

fn trim_blanks_end(text: &str) -> &str {
    text.trim_end_matches([' ', '\t'])
}

pub fn parse_tree_view(src: &str) -> Result<TreeViewAst, Error> {
    let mut r = Reader::<Token>::new(src, Kind::TreeView)?;
    let mut ast = TreeViewAst::default();
    r.expect(Token::Keyword)?;
    while r.heading(&mut ast.header) {}
    while !r.at_end() {
        let indent = r.at(Token::Indentation).then(|| r.take().len());
        let name = match r.kind() {
            Some(Token::QuotedName) => inner(r.take()).to_string(),
            Some(Token::BareName) => trim_blanks_end(r.take()).to_string(),
            _ => return Err(r.fail("a name")),
        };
        let mut node = TreeNodeAst {
            indent,
            name,
            class_annotation: None,
            icon_annotation: None,
            desc_annotation: None,
        };
        loop {
            match r.kind() {
                Some(Token::ClassAnnotation) => {
                    node.class_annotation = Some(js::trim(&js::trim(r.take())[3..]).to_string());
                }
                Some(Token::IconAnnotation) => {
                    let trimmed = js::trim(r.take());
                    node.icon_annotation = Some(trimmed[5..trimmed.len() - 1].to_string());
                }
                Some(Token::DescAnnotation) => {
                    node.desc_annotation = Some(js::trim(&js::trim(r.take())[2..]).to_string());
                }
                _ => break,
            }
        }
        ast.nodes.push(node);
    }
    Ok(ast)
}
