use std::{fs, path::PathBuf};

use icons::Icon;

fn assets() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets")
}

fn ported() -> Vec<(String, Vec<u8>)> {
    let mut glyphs = Vec::new();
    for entry in fs::read_dir(assets()).expect("the build script did not write assets/") {
        let path = entry.expect("reading assets/").path();
        if path.extension().is_some_and(|extension| extension == "svg") {
            let name = path.file_stem().unwrap().to_str().unwrap().to_owned();
            glyphs.push((name, fs::read(&path).expect("reading a glyph")));
        }
    }
    glyphs.sort_by(|a, b| a.0.cmp(&b.0));
    assert!(!glyphs.is_empty(), "no glyphs were ported");
    glyphs
}

#[test]
fn every_glyph_is_a_rooted_document() {
    for (name, bytes) in ported() {
        let text = std::str::from_utf8(&bytes).expect("a glyph that is not utf-8");
        assert!(text.starts_with("<svg"), "{name} is not an svg");
        // Everything below is about the root element, which is the only place
        // these attributes mean what the assertions take them to mean: a body
        // may carry its own `width` on a `<rect>`, and legitimately does.
        let root = &text[..text.find('>').expect("{name} has no root element")];
        assert!(root.contains("viewBox"), "{name} lacks a viewBox");
        // Without this the stroked paths fill solid.
        assert!(root.contains("fill="), "{name} lacks a root fill");
        // gpui sizes the element; a fixed width would fight it. The space
        // matters — `stroke-width` is not what this is looking for.
        assert!(!root.contains(" width="), "{name} carries a fixed width");
    }
}

/// The checks above prove a glyph *looks* like markup. gpui paints through
/// resvg, so this asserts the same parser gets geometry out of every one — the
/// failure it guards is a silent blank square, which no assertion on bytes sees.
#[test]
fn every_glyph_renders_through_the_parser_gpui_uses() {
    for (name, bytes) in ported() {
        let tree = usvg::Tree::from_data(&bytes, &usvg::Options::default())
            .unwrap_or_else(|e| panic!("{name} does not parse: {e}"));
        // Either axis, not both: `minus` is one horizontal stroke, so its
        // geometry is legitimately zero-height before the stroke is applied.
        let box_ = tree.root().abs_bounding_box();
        assert!(
            box_.width() > 0.0 || box_.height() > 0.0,
            "{name} renders to nothing",
        );
    }
}

/// The constants are generated from the same directory the assets are, so this
/// is what holds the two halves together: a rename upstream that missed one
/// side shows up here rather than as a missing icon at runtime.
#[test]
fn a_constant_carries_its_asset() {
    let named: Vec<(&str, &[u8])> = vec![
        ("search", icons::glyph::Search),
        ("play", icons::glyph::Play),
        ("trash", icons::glyph::Trash),
        ("chevron-left", icons::glyph::ChevronLeft),
    ];
    for (name, glyph) in named {
        let asset = fs::read(assets().join(format!("{name}.svg"))).expect("a named asset");
        assert_eq!(glyph, asset.as_slice(), "{name} does not carry its asset");
    }
}

/// An icon Lucide files under two categories has to stay one constant, or the
/// bytes land in the binary twice.
#[test]
fn a_shared_glyph_is_one_constant() {
    // `play` is filed under both `arrows` and `multimedia` upstream.
    assert_eq!(
        icons::arrows::Play.as_ptr(),
        icons::multimedia::Play.as_ptr(),
        "a shared glyph was duplicated rather than re-exported",
    );
}

#[test]
fn the_catalog_covers_every_ported_glyph() {
    use std::collections::BTreeSet;

    let catalogued: BTreeSet<&str> = icons::CATEGORIES
        .iter()
        .flat_map(|(_, icons)| icons.iter().map(|(name, _)| *name))
        .collect();
    let on_disk: BTreeSet<String> = ported().into_iter().map(|(name, _)| name).collect();

    for (name, contents) in icons::CATEGORIES {
        assert!(!contents.is_empty(), "category {name} is empty");
    }
    // Under `full` every ported glyph is filed somewhere, so the catalogue and
    // the directory have to agree exactly.
    let catalogued: std::collections::BTreeSet<String> =
        catalogued.into_iter().map(str::to_owned).collect();
    assert_eq!(catalogued, on_disk, "the catalogue and assets/ disagree");
}

/// The solid twin has to keep the outline's geometry, or a control swapping
/// between the two jumps as it redraws.
#[test]
fn a_solid_twin_is_its_outline_painted_in() {
    let outline = std::str::from_utf8(icons::glyph::Heart).unwrap();
    assert!(outline.contains(r#"fill="none""#), "the outline is filled");

    let filled = Icon::glyph(icons::glyph::Heart)
        .solid()
        .data()
        .expect("a glyph carries its own document");
    let filled = std::str::from_utf8(&filled).unwrap();
    assert!(
        filled.contains(r#"fill="currentColor""#),
        "the solid twin is not filled"
    );

    let body = |svg: &str| svg[svg.find('>').unwrap()..].to_owned();
    assert_eq!(body(outline), body(filled), "the solid twin moved the path");
}

/// A component takes an [`Icon`] and never learns which kind it was handed.
/// Only the renderer asks, and this is what it asks.
#[test]
fn an_icon_erases_where_the_drawing_came_from() {
    assert_eq!(
        Icon::from(icons::glyph::Heart).data().as_deref(),
        Some(icons::glyph::Heart),
        "a glyph lost its document"
    );
    // Resolved by the app's own `AssetSource` at paint time, so there is
    // nothing for this side to hand a parser.
    assert_eq!(Icon::path("brand/mark.svg").data(), None);
}
