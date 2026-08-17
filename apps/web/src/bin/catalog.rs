//! Dumps [`gallery::TABS`] as JSON for the website to build its nav from.
//!
//! The rail and the web sidebar read the same catalog, so a component cannot
//! appear on the site without existing in the gallery — the invariant that
//! keeps the docs from drifting is a build step, not a habit.

use gallery::TABS;

fn main() {
    let mut out = String::from("[");
    for (i, tab) in TABS.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        out.push_str(&format!(
            "{{\"title\":{},\"home\":{},\"fullBleed\":{},\"groups\":[",
            quote(tab.title),
            quote(tab.home),
            tab.full_bleed
        ));
        for (j, group) in tab.groups.iter().enumerate() {
            if j > 0 {
                out.push(',');
            }
            out.push_str(&format!(
                "{{\"title\":{},\"sections\":[",
                quote(group.title)
            ));
            for (k, section) in group.sections.iter().enumerate() {
                if k > 0 {
                    out.push(',');
                }
                let source = match section.source {
                    Some(path) => quote(path),
                    None => "null".to_string(),
                };
                out.push_str(&format!(
                    "{{\"key\":{},\"title\":{},\"source\":{}}}",
                    quote(section.key),
                    quote(section.title),
                    source
                ));
            }
            out.push_str("]}");
        }
        out.push_str("]}");
    }
    out.push(']');
    println!("{out}");
}

fn quote(s: &str) -> String {
    let escaped = s.replace('\\', "\\\\").replace('"', "\\\"");
    format!("\"{escaped}\"")
}
