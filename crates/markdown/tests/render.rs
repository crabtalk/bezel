use gpui::FontWeight;
use markdown::{render::flatten, *};
use theme::Theme;
#[test]
fn flatten_cuts_the_text_at_every_mark_boundary() {
    let theme = Theme::dark();
    let doc = parse("plain **bold** `code` tail");
    let BlockKind::Paragraph(text) = &doc.blocks[0].kind else {
        panic!("expected a paragraph")
    };
    let flat = flatten(text, FontWeight::NORMAL, &theme);

    // The runs must cover the text exactly, or gpui shapes the wrong bytes.
    assert_eq!(
        flat.runs.iter().map(|run| run.len).sum::<usize>(),
        flat.text.len()
    );
    assert_eq!(flat.code.len(), 1);
    assert!(
        flat.runs
            .iter()
            .any(|run| run.font.weight == FontWeight::SEMIBOLD)
    );
}

#[test]
fn runs_cover_the_text_for_every_shape_of_mark() {
    let theme = Theme::dark();
    for source in [
        "**_nested_**",
        "a [link](u) b",
        "![alt](u) trailing",
        "~~struck~~ and `mono`",
        "**bold `code` inside**",
        "no marks at all",
        "",
    ] {
        for block in parse(source).blocks {
            let Some(text) = block.text_at(Part::Body) else {
                continue;
            };
            let flat = flatten(text, FontWeight::NORMAL, &theme);
            assert_eq!(
                flat.runs.iter().map(|run| run.len).sum::<usize>(),
                flat.text.len(),
                "runs do not cover {source:?}"
            );
        }
    }
}

#[test]
fn adjacent_links_merge_into_one_clickable_range() {
    let theme = Theme::dark();
    let doc = parse("[**bold** and plain](https://example.com)");
    let BlockKind::Paragraph(text) = &doc.blocks[0].kind else {
        panic!("expected a paragraph")
    };
    let flat = flatten(text, FontWeight::NORMAL, &theme);
    assert_eq!(flat.links.len(), 1);
    assert_eq!(flat.links[0].0, 0..text.text.len());
}

#[test]
fn a_relative_image_path_joins_the_base_and_nothing_else_does() {
    use gpui::{ImageSource, Resource};
    use std::path::Path;

    let resolve = |url: &str, base: Option<&str>| match image_source(url, base.map(Path::new)) {
        ImageSource::Resource(Resource::Path(path)) => path.to_string_lossy().into_owned(),
        ImageSource::Resource(Resource::Uri(uri)) => uri.to_string(),
        _ => panic!("expected a path or a URI"),
    };
    let base = Some("/notes/article");
    assert_eq!(
        resolve("assets/shot.png", base),
        Path::new("/notes/article")
            .join("assets/shot.png")
            .to_string_lossy()
    );
    assert_eq!(resolve("assets/shot.png", None), "assets/shot.png");
    assert_eq!(resolve("/pics/shot.png", base), "/pics/shot.png");
    assert_eq!(resolve("https://x.dev/a.png", base), "https://x.dev/a.png");
}
