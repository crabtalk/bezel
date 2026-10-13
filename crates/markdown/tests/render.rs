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

#[test]
fn a_mention_shows_its_host_behind_a_favicon_slot() {
    let theme = Theme::dark();
    let doc = parse("a [https://x.com/p](https://x.com/p \"chip\") b");
    let BlockKind::Paragraph(text) = &doc.blocks[0].kind else {
        panic!("expected a paragraph")
    };
    let flat = flatten(text, FontWeight::NORMAL, &theme);
    assert_eq!(flat.text.as_ref(), "a \u{2003} x.com b");
    assert_eq!(
        flat.runs.iter().map(|run| run.len).sum::<usize>(),
        flat.text.len()
    );
    let mention = &flat.mentions[0];
    let end = 2 + "https://x.com/p".len();
    assert_eq!(flat.shown.at(2), mention.range.start);
    assert_eq!(flat.shown.at(end), mention.range.end);
    assert_eq!(flat.shown.at(end + 2), mention.range.end + 2);
    assert_eq!(flat.shown.offset(mention.range.start + 1), 2);
    assert_eq!(flat.shown.offset(mention.range.end - 1), end);
    assert_eq!(flat.shown.offset(mention.range.end + 2), end + 2);
}

#[test]
fn a_mark_background_is_a_wash_and_not_a_run_background() {
    let theme = Theme::dark();
    let yellow: gpui::Hsla = gpui::rgb(0xffd60a).into();
    let marks = Marks::new().with("highlight", "==");
    let doc = parse_with("plain ==marked== tail", &marks);
    let BlockKind::Paragraph(text) = &doc.blocks[0].kind else {
        panic!("expected a paragraph")
    };
    let flat = markdown::render::flatten_with(
        text,
        FontWeight::NORMAL,
        &theme,
        |name| {
            (name == "highlight").then_some(MarkPaint {
                background: Some(yellow),
                ..Default::default()
            })
        },
        |_| None,
    );

    // A run's background is painted with the text, over the selection.
    assert!(flat.runs.iter().all(|run| run.background_color.is_none()));
    assert_eq!(flat.washes, vec![(6..12, yellow)]);
    assert_eq!(&flat.text[6..12], "marked");
}
