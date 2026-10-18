//! Mermaid's box-drawing preprocessor: turns `├──`, `└──`, `│` (and the
//! heavy variants) into the indentation the grammar reads. Pele's
//! `diagrams/treeview/boxDrawing.ts`.

use indexmap::IndexMap;

use crate::mermaid::{Error, Kind, js, js::Lazy};

const INDENT_UNIT: &str = "    ";

static ALL_BOX_CHARS: Lazy = Lazy::new(r"[─━│┃└┗├┣]", "");
static BRANCH_CHAR: Lazy = Lazy::new(r"[└┗├┣]", "");
static DASH_CHAR: Lazy = Lazy::new(r"[─━]", "");
static DECORATION_ONLY: Lazy = Lazy::new(r"^[\s│┃]+$", "");
static BOX_ONLY: Lazy = Lazy::new(r"^[\s─━│┃└┗├┣]+$", "");
static METADATA_LINE: Lazy = Lazy::new(r"^\s*(title[\t ]|accTitle[\t ]*:|accDescr[\t ]*[:{])", "");
static COMMENT_LINE: Lazy = Lazy::new(r"^\s*%%", "");
static RE_LINE: Lazy = Lazy::new(r"\bline\s+(\d+)\b", "gi");
static RE_INDENTED: Lazy = Lazy::new(r"^\s+", "");

pub struct PreprocessResult {
    pub text: String,
    /// Output line number to original line number, both from 1. Empty when
    /// the text is unchanged.
    pub line_map: IndexMap<usize, usize>,
}

pub fn is_box_drawing_format(lines: &[String]) -> bool {
    lines.iter().any(|line| ALL_BOX_CHARS.test(line))
}

fn infer_segment_width(content_lines: &[String]) -> usize {
    for line in content_lines {
        if let Some(m) = BRANCH_CHAR.exec(line) {
            let index = js::len(&line[..m.start()]);
            if index > 0 {
                return index;
            }
        }
    }
    4
}

pub fn remap_error_lines(message: &str, line_map: &IndexMap<usize, usize>) -> String {
    RE_LINE.replace(message, |caps| {
        match line_map.get(&(js::parse_int(caps.str(1), 10) as usize)) {
            Some(&original) if original != 0 => format!("line {original}"),
            _ => caps.str(0).to_string(),
        }
    })
}

fn fail(line: usize, message: &str) -> Error {
    let mut error = Error::syntax(format!("Line {line}: {message}")).of(Kind::TreeView);
    error.line = line;
    error
}

pub fn preprocess_box_drawing(input: &str) -> Result<PreprocessResult, Error> {
    let mut line_map = IndexMap::new();
    let unchanged = |line_map| {
        Ok(PreprocessResult {
            text: input.to_string(),
            line_map,
        })
    };
    if !ALL_BOX_CHARS.test(input) {
        return unchanged(line_map);
    }
    let lines: Vec<&str> = input.split('\n').collect();

    let Some(keyword_idx) = lines
        .iter()
        .position(|line| js::trim(line) == "treeView-beta")
    else {
        return unchanged(line_map);
    };

    let passes = |line: &str| -> bool {
        js::trim(line).is_empty() || COMMENT_LINE.test(line) || METADATA_LINE.test(line)
    };

    let mut content_lines: Vec<String> = Vec::new();
    for line in &lines[keyword_idx + 1..] {
        if passes(line) || DECORATION_ONLY.test(line) {
            continue;
        }
        content_lines.push(line.replace('\t', INDENT_UNIT));
    }
    if !is_box_drawing_format(&content_lines) {
        return unchanged(line_map);
    }

    let segment_width = infer_segment_width(&content_lines);
    let mut out: Vec<String> = Vec::new();
    let mut keep = |text: String, index: usize, line_map: &mut IndexMap<usize, usize>| {
        out.push(text);
        line_map.insert(out.len(), index + 1);
    };

    for (i, line) in lines.iter().enumerate().take(keyword_idx + 1) {
        keep(line.to_string(), i, &mut line_map);
    }
    for (i, &line) in lines.iter().enumerate().skip(keyword_idx + 1) {
        if passes(line) {
            keep(line.to_string(), i, &mut line_map);
            continue;
        }
        if DECORATION_ONLY.test(line) {
            continue;
        }

        let normalized = line.replace('\t', INDENT_UNIT);
        if let Some(branch) = BRANCH_CHAR.exec(&normalized) {
            let index = js::len(&normalized[..branch.start()]);
            let depth = js::round(index as f64 / segment_width as f64) as usize + 1;
            let mut pos = branch.end();
            while pos < normalized.len() {
                let width = crate::mermaid::util::chars::width_at(&normalized, pos);
                if !DASH_CHAR.test(&normalized[pos..pos + width]) {
                    break;
                }
                pos += width;
            }
            while normalized.as_bytes().get(pos) == Some(&b' ') {
                pos += 1;
            }
            let content = js::trim_end(&normalized[pos..]);
            if content.is_empty() {
                return Err(fail(
                    i + 1,
                    "Empty node — expected a filename or directory name after the box-drawing prefix",
                ));
            }
            keep(INDENT_UNIT.repeat(depth) + content, i, &mut line_map);
        } else if BOX_ONLY.test(&normalized) {
            continue;
        } else if ALL_BOX_CHARS.test(&normalized) {
            // A name that happens to contain a box character, such as
            // "Section ─ A.txt", is a root item.
            keep(line.to_string(), i, &mut line_map);
        } else if RE_INDENTED.test(&normalized) {
            return Err(fail(
                i + 1,
                "Unexpected indentation without box-drawing characters. In box-drawing format, use ├── or └── prefixes for indented nodes.",
            ));
        } else {
            keep(line.to_string(), i, &mut line_map);
        }
    }
    Ok(PreprocessResult {
        text: out.join("\n"),
        line_map,
    })
}
