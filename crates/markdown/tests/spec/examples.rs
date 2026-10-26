//! The `example` blocks of a CommonMark-format spec file, shared by the
//! conformance test and the bench.
#![allow(dead_code)]

const FENCE: &str = "````````````````````````````````";

/// One `example` block of a spec file.
pub struct Example {
    pub number: usize,
    pub section: String,
    pub markdown: String,
    pub html: String,
}

/// The examples of a spec file whose block names one of `kinds` after
/// `example`, `""` for a plain one, numbered as the spec numbers them.
pub fn examples(spec: &str, kinds: &[&str]) -> Vec<Example> {
    let mut out = Vec::new();
    let mut section = String::new();
    let mut lines = spec.lines();
    let mut number = 0;
    while let Some(line) = lines.next() {
        if let Some(heading) = line.strip_prefix("## ").or(line.strip_prefix("# ")) {
            section = heading.trim().to_string();
            continue;
        }
        let Some(kind) = line
            .strip_prefix(FENCE)
            .and_then(|rest| rest.strip_prefix(" example"))
        else {
            continue;
        };
        number += 1;
        let (mut markdown, mut html) = (String::new(), String::new());
        let mut into_html = false;
        for line in lines.by_ref() {
            if line == FENCE {
                break;
            }
            if line == "." && !into_html {
                into_html = true;
                continue;
            }
            let target = if into_html { &mut html } else { &mut markdown };
            target.push_str(&line.replace('→', "\t"));
            target.push('\n');
        }
        if kinds.contains(&kind.trim()) {
            out.push(Example {
                number,
                section: section.clone(),
                markdown,
                html,
            });
        }
    }
    out
}
