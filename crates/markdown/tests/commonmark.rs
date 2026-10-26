//! [`markdown::html`] against the CommonMark spec's examples and GFM's
//! extension examples, from the files `tests/spec/fetch.sh` downloads. Without
//! them the test passes and says so.
//!
//! `tests/spec/failing` lists the examples known to fail. The test fails when
//! one not on it fails, or one on it passes. `SPEC_BLESS=1` rewrites the list
//! from the run, and `SPEC_DUMP=1` prints each failing example with both
//! renderings.
//!
//! What fails by the document model's shape: raw HTML is written as text, a
//! link's title is not kept, a list is never loose, a quote inside a quote is
//! one quote, adjacent lists of two markers are one list, and a picture stands
//! in a `<figure>`. The parse linkifies no bare `www.` or email address.

use std::{collections::BTreeSet, env, fs, path::Path};

#[path = "spec/examples.rs"]
mod examples;

/// What two renderings of one document share: no newline against a tag
/// outside a `<pre>`, no self-closing slash, none of this crate's own classes
/// or heading ids, a column's alignment as a style, and a line break — `<br>`
/// or a newline — as one newline. The document model keeps no difference
/// between a soft and a hard break.
fn normalize(html: &str) -> String {
    let mut out = String::new();
    let mut rest = html;
    let mut pre = 0usize;
    while let Some(open) = rest.find('<') {
        let text = &rest[..open];
        out.push_str(if pre > 0 {
            text
        } else {
            text.trim_matches('\n')
        });
        let close = rest[open..]
            .find('>')
            .map_or(rest.len(), |at| open + at + 1);
        let mut tag = rest[open..close].replace(" />", ">").replace("/>", ">");
        rest = &rest[close..];
        if tag == "<br>" && pre == 0 {
            out.push('\n');
            rest = rest.strip_prefix('\n').unwrap_or(rest);
            continue;
        }
        if tag.starts_with("<pre") {
            pre += 1;
        } else if tag == "</pre>" {
            pre = pre.saturating_sub(1);
        }
        for attribute in [" class=\"md-", " id=\""] {
            while let Some(at) = tag.find(attribute) {
                let end = tag[at + attribute.len()..]
                    .find('"')
                    .map_or(tag.len(), |end| at + attribute.len() + end + 1);
                tag.replace_range(at..end, "");
            }
        }
        for align in ["left", "center", "right"] {
            tag = tag.replace(
                &format!(" align=\"{align}\""),
                &format!(" style=\"text-align:{align}\""),
            );
        }
        out.push_str(&tag);
    }
    out.push_str(rest.trim_matches('\n'));
    out
}

fn render(markdown: &str) -> String {
    normalize(&markdown::html(&markdown::parse(markdown)))
}

#[test]
fn spec_examples() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/spec");
    let suites: [(&str, &[&str]); 2] = [
        ("commonmark", &[""]),
        // GFM's task list examples are marked `disabled`.
        ("gfm", &["table", "strikethrough", "autolink", "disabled"]),
    ];
    let mut failing = BTreeSet::new();
    let mut ran = Vec::new();
    for (suite, kinds) in suites {
        let Ok(spec) = fs::read_to_string(dir.join(format!("{suite}.txt"))) else {
            eprintln!("no {suite}.txt: run crates/markdown/tests/spec/fetch.sh");
            continue;
        };
        let examples = examples::examples(&spec, kinds);
        let mut failed = 0;
        for example in &examples {
            let ours = render(&example.markdown);
            let want = normalize(&example.html);
            if ours == want {
                continue;
            }
            failed += 1;
            failing.insert(format!("{suite} {}", example.number));
            if env::var_os("SPEC_DUMP").is_some() {
                eprintln!(
                    "--- {suite} {} ({}) ---\n{}want: {want}\nours: {ours}\n",
                    example.number, example.section, example.markdown
                );
            }
        }
        eprintln!(
            "{suite}: {} of {} pass",
            examples.len() - failed,
            examples.len()
        );
        ran.push(suite);
    }
    if ran.is_empty() {
        return;
    }

    let list = dir.join("failing");
    if env::var_os("SPEC_BLESS").is_some() {
        let mut written: String = failing.iter().map(|line| format!("{line}\n")).collect();
        if written.is_empty() {
            written.push('\n');
        }
        fs::write(&list, written).expect("the failing list");
        return;
    }
    let known: BTreeSet<String> = fs::read_to_string(&list)
        .expect("tests/spec/failing")
        .lines()
        .filter(|line| {
            ran.iter()
                .any(|suite| line.starts_with(&format!("{suite} ")))
        })
        .map(str::to_owned)
        .collect();
    let regressed: Vec<_> = failing.difference(&known).collect();
    let fixed: Vec<_> = known.difference(&failing).collect();
    assert!(
        regressed.is_empty() && fixed.is_empty(),
        "now failing: {regressed:?}\nnow passing, off the list: {fixed:?}\n\
         SPEC_DUMP=1 shows each failure; SPEC_BLESS=1 rewrites tests/spec/failing"
    );
}
