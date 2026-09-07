use gpui::AssetSource;
use icons::{Assets, CATEGORIES, PATHS};

#[test]
fn every_registered_icon_loads_and_parses() {
    for path in Assets.list("icons/").unwrap() {
        let bytes = Assets
            .load(&path)
            .unwrap()
            .unwrap_or_else(|| panic!("missing asset {path}"));
        let text = std::str::from_utf8(&bytes).expect("icon svg is utf-8");
        assert!(text.contains("<svg"), "{path} is not an svg");
        assert!(text.contains("viewBox"), "{path} lacks a viewBox");
        // Without this the stroked paths fill solid: the attribute lives on the
        // root, and `icondata` hands us the body without one.
        assert!(text.contains("fill="), "{path} lacks a root fill");
    }
}

#[test]
fn unknown_paths_are_none() {
    assert!(Assets.load("icons/nope.svg").unwrap().is_none());
}

#[test]
fn list_filters_by_prefix() {
    assert!(!Assets.list("icons/").unwrap().is_empty());
    assert!(Assets.list("fonts/").unwrap().is_empty());
    // A category is a path prefix too, which is what makes the set browsable
    // one group at a time.
    assert!(
        Assets
            .list("icons/arrows/")
            .unwrap()
            .iter()
            .all(|p| p.starts_with("icons/arrows/"))
    );
}

#[test]
fn every_category_is_populated_and_listed() {
    let counted: usize = CATEGORIES.iter().map(|(_, icons)| icons.len()).sum();
    assert_eq!(counted, PATHS.len(), "a category is missing from PATHS");

    for (name, contents) in CATEGORIES {
        assert!(!contents.is_empty(), "category {name} is empty");
        for (constant, path) in *contents {
            assert!(
                path.starts_with(&format!("icons/{name}/")),
                "{constant} is filed under {name} but served from {path}",
            );
        }
    }
}

#[test]
fn a_bold_twin_is_its_linear_twin_painted_solid() {
    let linear = Assets.load(icons::media::PLAY).unwrap().unwrap();
    let bold = Assets.load(icons::media::PLAY_BOLD).unwrap().unwrap();
    let linear = std::str::from_utf8(&linear).unwrap();
    let bold = std::str::from_utf8(&bold).unwrap();

    assert!(
        linear.contains(r#"fill="none""#),
        "the outline twin is filled"
    );
    assert!(
        bold.contains(r#"fill="currentColor""#),
        "the bold twin is hollow"
    );
    // Same geometry, so a control swapping between them does not jump.
    let body = |svg: &str| svg[svg.find('>').unwrap()..].to_string();
    assert_eq!(body(linear), body(bold));
}

/// The string checks above prove a document *looks* like markup. gpui paints
/// through resvg, so this asserts the same parser gets geometry out of every
/// icon — the failure this guards is a silent blank square, which no assertion
/// on bytes can see.
#[test]
fn every_icon_renders_through_the_parser_gpui_uses() {
    for path in PATHS {
        let bytes = Assets.load(path).unwrap().unwrap();
        let tree = usvg::Tree::from_data(&bytes, &usvg::Options::default())
            .unwrap_or_else(|e| panic!("{path} does not parse: {e}"));
        let box_ = tree.root().abs_bounding_box();
        assert!(
            box_.width() > 0.0 && box_.height() > 0.0,
            "{path} renders to nothing",
        );
    }
}

/// An icon writes its name twice — `(MAGNIFER, "magnifer", LuSearch)` — because
/// only a procedural macro can mint an identifier from a string, and the set is
/// not worth a second published crate. This is the half of that trade that buys
/// the safety back: the constant and the file name have to agree.
///
/// The drift is not hypothetical. Before this crate existed the set had
/// `(DOWNLOAD, "download-minimalistic")` and `(LINK, "link-minimalistic")` —
/// two constants whose names had quietly stopped describing their assets.
#[test]
fn constants_match_their_paths() {
    for (category, contents) in CATEGORIES {
        for (constant, path) in *contents {
            let stem = path
                .strip_prefix(&format!("icons/{category}/"))
                .and_then(|rest| rest.strip_suffix(".svg"))
                .unwrap_or_else(|| panic!("{path} is not filed under {category}"));
            assert_eq!(
                *constant,
                stem.to_uppercase().replace('-', "_"),
                "{constant} does not name {path}",
            );
        }
    }
}
