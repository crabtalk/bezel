use gpui::AssetSource;

use ui::icons::*;

#[test]
fn every_registered_icon_loads_and_parses() {
    let assets = Assets;
    for path in assets.list("icons/").unwrap() {
        let bytes = assets
            .load(&path)
            .unwrap()
            .unwrap_or_else(|| panic!("missing asset {path}"));
        let text = std::str::from_utf8(&bytes).expect("icon svg is utf-8");
        assert!(text.contains("<svg"), "{path} is not an svg");
        assert!(text.contains("viewBox"), "{path} lacks a viewBox");
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
}
