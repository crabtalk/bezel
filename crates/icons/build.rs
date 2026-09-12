//! Ports the pinned Lucide release into `assets/`, and writes the module tree
//! that names it into `src/generated.rs`.
//!
//! Both outputs are gitignored and both are bundled into the published crate by
//! `include` in `Cargo.toml`, so the download happens only in a fresh checkout.
//! A consumer building from crates.io finds the glyphs already there, which is
//! what keeps the crate offline, vendorable and buildable on docs.rs.
//!
//! Bumping `LUCIDE` is the upgrade: the index stops matching and the next build
//! re-ports, so the diff shows exactly which glyphs upstream redrew.

use std::{collections::BTreeMap, fs, path::Path, process::Command};

const LUCIDE: &str = "1.45.0";

/// The ported version and each glyph's categories, kept beside the assets so a
/// build can regenerate the module tree without reaching for the network.
const INDEX: &str = "index.txt";

fn main() {
    println!("cargo:rerun-if-changed=build.rs");

    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let assets = crate_dir.join("assets");
    let index = assets.join(INDEX);

    let filed = match read_index(&index) {
        Some(filed) => filed,
        None => {
            let bundle = fetch(&Path::new(&std::env::var("OUT_DIR").unwrap()).join("lucide"));
            port(&bundle, &assets, &index)
        }
    };

    // Always regenerated, never cached: it is derived from the index alone, so
    // editing this file is enough to change what the crate exposes.
    fs::write(crate_dir.join("src/generated.rs"), source(&filed))
        .expect("writing the generated module tree");
}

/// The index, if one was written by this same Lucide release.
fn read_index(index: &Path) -> Option<BTreeMap<String, Vec<String>>> {
    let text = fs::read_to_string(index).ok()?;
    let mut lines = text.lines();
    (lines.next()? == LUCIDE).then_some(())?;

    let filed = lines
        .filter_map(|line| {
            let (name, categories) = line.split_once('\t')?;
            Some((
                name.to_owned(),
                categories.split(',').map(str::to_owned).collect(),
            ))
        })
        .collect::<BTreeMap<String, Vec<String>>>();
    (!filed.is_empty()).then_some(filed)
}

/// Downloads and unpacks the release, returning the directory holding the pairs
/// of `<name>.svg` and `<name>.json` that Lucide ships.
fn fetch(work: &Path) -> std::path::PathBuf {
    let icons = work.join("icons");
    if icons.is_dir() {
        return icons;
    }
    fs::create_dir_all(work).expect("creating the download directory");

    let zip = work.join("lucide.zip");
    let url = format!(
        "https://github.com/lucide-icons/lucide/releases/download/{LUCIDE}/lucide-icons-{LUCIDE}.zip"
    );
    run(
        Command::new("curl")
            .args(["-sSL", "--fail", "-o"])
            .arg(&zip)
            .arg(&url),
        "curl",
    );
    // `unzip` wherever it exists, `tar` for the Windows boxes that ship bsdtar
    // without it. One of the two is on every platform the matrix runs.
    let unzip = Command::new("unzip")
        .arg("-qo")
        .arg(&zip)
        .arg("-d")
        .arg(work)
        .status();
    if !unzip.is_ok_and(|status| status.success()) {
        run(
            Command::new("tar").arg("-xf").arg(&zip).arg("-C").arg(work),
            "tar",
        );
    }

    assert!(icons.is_dir(), "the bundle held no icons/ directory");
    icons
}

fn run(command: &mut Command, name: &str) {
    let status = command
        .status()
        .unwrap_or_else(|e| panic!("could not run `{name}`, needed to port the Lucide icons: {e}"));
    assert!(
        status.success(),
        "`{name}` failed while porting the Lucide icons"
    );
}

/// Rewrites every glyph into the document gpui paints and records what Lucide
/// files it under, returning that filing.
fn port(bundle: &Path, assets: &Path, index: &Path) -> BTreeMap<String, Vec<String>> {
    let _ = fs::remove_dir_all(assets);
    fs::create_dir_all(assets).expect("creating the assets directory");

    let mut names: Vec<String> = fs::read_dir(bundle)
        .expect("reading the bundle")
        .filter_map(|entry| {
            let path = entry.ok()?.path();
            if path.extension()? != "svg" {
                return None;
            }
            Some(path.file_stem()?.to_str()?.to_owned())
        })
        .collect();
    names.sort();
    assert!(!names.is_empty(), "the bundle held no glyphs");

    let mut filed = BTreeMap::new();
    for name in names {
        let svg = fs::read_to_string(bundle.join(format!("{name}.svg"))).expect("reading a glyph");
        fs::write(assets.join(format!("{name}.svg")), document(&svg)).expect("writing a glyph");

        let meta = fs::read_to_string(bundle.join(format!("{name}.json"))).unwrap_or_default();
        let categories = list(&meta, "categories");
        assert!(!categories.is_empty(), "{name} is in no category");
        filed.insert(name, categories);
    }

    // Sorted, one glyph per line, so a re-port with nothing new upstream leaves
    // a byte-identical file.
    let mut text = format!("{LUCIDE}\n");
    for (name, categories) in &filed {
        text.push_str(&format!("{name}\t{}\n", categories.join(",")));
    }
    fs::write(index, text).expect("writing the index");

    filed
}

/// The SVG gpui renders: Lucide's root attributes minus `width`/`height`, which
/// would fight the size the caller sets on the element.
fn document(svg: &str) -> String {
    const KEEP: [&str; 6] = [
        "viewBox",
        "fill",
        "stroke",
        "stroke-width",
        "stroke-linecap",
        "stroke-linejoin",
    ];

    let open = svg.find("<svg").expect("a glyph with no root element");
    let close = svg[open..].find('>').expect("an unterminated root element") + open;
    let attributes = attributes(&svg[open + 4..close]);
    let body = svg[close + 1..svg.rfind("</svg>").expect("an unclosed root")]
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    assert!(!body.is_empty(), "a glyph with an empty body");

    let mut out = String::from("<svg xmlns=\"http://www.w3.org/2000/svg\"");
    for key in KEEP {
        if let Some(value) = attributes.get(key) {
            out.push_str(&format!(" {key}=\"{value}\""));
        }
    }
    out.push('>');
    out.push_str(&body);
    out.push_str("</svg>");
    out
}

fn attributes(open: &str) -> BTreeMap<String, String> {
    let mut found = BTreeMap::new();
    let mut rest = open;
    while let Some(equals) = rest.find('=') {
        let key = rest[..equals]
            .trim()
            .rsplit(char::is_whitespace)
            .next()
            .unwrap_or_default();
        let Some(start) = rest[equals..].find('"').map(|i| equals + i + 1) else {
            break;
        };
        let Some(end) = rest[start..].find('"').map(|i| start + i) else {
            break;
        };
        if !key.is_empty() {
            found.insert(key.to_owned(), rest[start..end].to_owned());
        }
        rest = &rest[end + 1..];
    }
    found
}

/// Pulls a JSON array of strings out of an icon's metadata. Lucide's schema is
/// flat, so this costs less than a parser every consumer would compile.
fn list(json: &str, key: &str) -> Vec<String> {
    let Some(start) = json.find(&format!("\"{key}\"")) else {
        return Vec::new();
    };
    let Some(open) = json[start..].find('[').map(|i| start + i) else {
        return Vec::new();
    };
    let Some(close) = json[open..].find(']').map(|i| open + i) else {
        return Vec::new();
    };
    json[open + 1..close]
        .split(',')
        .filter_map(|item| {
            let item = item.trim().trim_matches('"').trim();
            (!item.is_empty()).then(|| item.to_owned())
        })
        .collect()
}

/// Lucide's own spelling for a glyph: `arrow-big-down` is `ArrowBigDown`, the
/// name their site and their React package use. It is not Rust's casing for a
/// constant, which is why the generated file allows the lint — copying a name
/// off lucide.dev and having it compile is worth more than the convention.
fn constant(name: &str) -> String {
    name.split('-')
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect()
}

fn source(filed: &BTreeMap<String, Vec<String>>) -> String {
    let mut categories: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for (name, of) in filed {
        for category in of {
            categories.entry(category).or_default().push(name);
        }
    }

    let mut out = format!(
        "// Generated by build.rs from Lucide {LUCIDE}. Do not edit.\n\
         //\n\
         // Names are Lucide's, which is PascalCase rather than Rust's casing for\n\
         // a constant: a name copied off lucide.dev should compile here.\n\
         #![allow(non_upper_case_globals)]\n\
         //\n\
         // Every glyph is defined once in `glyph`; a category re-exports the ones\n\
         // Lucide files under it, so a glyph in two categories stays one constant\n\
         // and one copy of the bytes.\n\n\
         /// Every enabled glyph, under Lucide's own name.\n\
         pub mod glyph {{\n"
    );
    for (name, of) in filed {
        // A glyph compiles when any category filing it is on: Lucide files
        // `play` under both arrows and multimedia, and either should paint it.
        let gate = match of.len() {
            1 => format!("feature = \"{}\"", of[0]),
            _ => format!(
                "any({})",
                of.iter()
                    .map(|category| format!("feature = \"{category}\""))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        };
        out.push_str(&format!(
            "    #[doc = \"Lucide `{name}` — {}.\"]\n    #[cfg({gate})]\n    pub const {}: &[u8] = include_bytes!(\"../assets/{name}.svg\");\n",
            of.join(", "),
            constant(name),
        ));
    }
    out.push_str("}\n");

    for (category, members) in &categories {
        out.push_str(&format!(
            "\n/// Lucide's `{category}` category — {} icons.\n#[cfg(feature = \"{category}\")]\npub mod {} {{\n",
            members.len(),
            category.replace('-', "_"),
        ));
        for name in members {
            out.push_str(&format!("    pub use crate::glyph::{};\n", constant(name)));
        }
        out.push_str("}\n");
    }

    // A `const` is codegen'd only where it is named, so this costs a binary
    // nothing until an icon browser reaches for it — and costs it every enabled
    // glyph the moment one does.
    out.push_str(
        "\n/// One category's icons, each a name and the SVG itself.\npub type Icons = &'static [(&'static str, &'static [u8])];\n\n/// Every enabled category and its icons.\npub const CATEGORIES: &[(&str, Icons)] = &[\n",
    );
    for (category, members) in &categories {
        out.push_str(&format!(
            "    #[cfg(feature = \"{category}\")]\n    (\"{category}\", &[\n"
        ));
        for name in members {
            out.push_str(&format!(
                "        (\"{name}\", crate::glyph::{}),\n",
                constant(name)
            ));
        }
        out.push_str("    ]),\n");
    }
    out.push_str("];\n");
    out
}
