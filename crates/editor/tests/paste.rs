use std::path::{Path, PathBuf};

use editor::{AppExt as _, Editor, ImageStore, Mode, PasteContent, Source};
use gpui::{
    App, ClipboardEntry, ClipboardItem, ClipboardString, Entity, EntityId, ExternalPaths,
    Focusable, Global, TestAppContext, VisualTestContext,
};
use markdown::{BlockKind, Part, Selection};

#[cfg(target_os = "macos")]
const PRIMARY: &str = "cmd";
#[cfg(not(target_os = "macos"))]
const PRIMARY: &str = "ctrl";

fn open(source: &str, mode: Mode, cx: &mut TestAppContext) -> (Entity<Editor>, VisualTestContext) {
    cx.update(|cx| {
        theme::Theme::install(theme::Appearance::Dark, cx);
        editor::init(cx);
    });
    let window = cx.add_window(|_, cx| Editor::new(source, cx).with_mode(mode).with_base("/notes"));
    let editor = window.root(cx).unwrap();
    let mut visual = VisualTestContext::from_window(window.into(), cx);
    visual.update(|window, cx| window.focus(&editor.read(cx).focus_handle(cx), cx));
    visual.run_until_parked();
    (editor, visual)
}

fn paste(item: ClipboardItem, cx: &mut VisualTestContext) {
    cx.update(|_, cx| cx.write_to_clipboard(item));
    cx.simulate_keystrokes(&format!("{PRIMARY}-v"));
}

fn source(editor: &Entity<Editor>, cx: &mut VisualTestContext) -> String {
    cx.update(|_, cx| editor.read(cx).source())
}

struct Destination {
    editor: EntityId,
    mode: Mode,
    in_fence: bool,
    calls: usize,
}
impl Global for Destination {}

#[gpui::test]
fn handler_receives_the_destination_and_literal_paste_is_one_edit(cx: &mut TestAppContext) {
    for (original, mode, in_fence) in [
        ("old", Mode::Blocks, false),
        ("```\nold\n```", Mode::Blocks, true),
        ("old", Mode::Source, true),
    ] {
        let (editor, mut visual) = open(original, mode, cx);
        visual.update(|_, cx| {
            cx.set_global(Destination {
                editor: editor.entity_id(),
                mode,
                in_fence,
                calls: 0,
            });
            cx.set_paste_handler(|item, editor, destination, cx| {
                assert_eq!(item.text().as_deref(), Some("clipboard"));
                let expected = cx.global_mut::<Destination>();
                assert_eq!(editor.entity_id(), expected.editor);
                assert_eq!(destination.mode, expected.mode);
                assert_eq!(destination.in_fence, expected.in_fence);
                assert_eq!(destination.base, Some(Path::new("/notes")));
                expected.calls += 1;
                Some(PasteContent::Literal("**literal**".into()))
            });
            editor.update(cx, |editor, cx| {
                editor.select(Selection::all(editor.doc()), cx)
            });
        });
        paste(ClipboardItem::new_string("clipboard".into()), &mut visual);
        visual.update(|_, cx| {
            let editor = editor.read(cx);
            let part = if in_fence { Part::Code } else { Part::Body };
            let text = editor.doc().blocks[0].text_at(part).unwrap();
            assert_eq!(text.text, "**literal**");
            assert!(text.marks.is_empty());
            assert_eq!(cx.global::<Destination>().calls, 1);
        });
        visual.simulate_keystrokes(&format!("{PRIMARY}-z"));
        assert_eq!(source(&editor, &mut visual), original);
        visual.simulate_keystrokes(&format!("{PRIMARY}-shift-z"));
        assert!(source(&editor, &mut visual).contains("literal"));
    }
}

#[gpui::test]
fn markdown_response_uses_the_editors_dialect(cx: &mut TestAppContext) {
    let (editor, mut visual) = open("", Mode::Blocks, cx);
    visual.update(|_, cx| {
        // Set the dialect on a fresh editor before it reads the paste.
        editor.update(cx, |editor, cx| {
            *editor =
                Editor::new("", cx).with_marks(markdown::Marks::new().with("highlight", "=="));
        });
        cx.set_paste_handler(|_, _, _, _| {
            Some(PasteContent::Markdown("# Title\n\n==marked==".into()))
        });
    });
    visual.update(|window, cx| window.focus(&editor.read(cx).focus_handle(cx), cx));
    paste(ClipboardItem::new_string("ignored".into()), &mut visual);
    visual.update(|_, cx| {
        let doc = editor.read(cx).doc();
        assert!(matches!(
            doc.blocks[0].kind,
            BlockKind::Heading { level: 1, .. }
        ));
        assert_eq!(doc.blocks[1].text_at(Part::Body).unwrap().marks.len(), 1);
    });
    assert_eq!(source(&editor, &mut visual), "# Title\n\n==marked==");
    visual.simulate_keystrokes(&format!("{PRIMARY}-z"));
    assert_eq!(source(&editor, &mut visual), "");
}

#[gpui::test]
fn markdown_response_stays_literal_in_source_and_fences(cx: &mut TestAppContext) {
    for (text, mode, expected) in [
        ("", Mode::Source, "# Heading"),
        ("```\n\n```", Mode::Blocks, "```\n# Heading\n```"),
    ] {
        let (editor, mut visual) = open(text, mode, cx);
        visual.update(|_, cx| {
            cx.set_paste_handler(|_, _, _, _| Some(PasteContent::Markdown("# Heading".into())))
        });
        paste(ClipboardItem::new_string("ignored".into()), &mut visual);
        assert_eq!(source(&editor, &mut visual), expected);
    }
}

struct Kept(usize);
impl Global for Kept {}

fn keep(source: Source, editor: &Entity<Editor>, base: Option<&Path>, cx: &App) -> Option<String> {
    assert_eq!(editor.entity_id(), cx.global::<Destination>().editor);
    assert_eq!(base, Some(Path::new("/notes")));
    assert!(matches!(source, Source::Bytes(_) | Source::File(_)));
    Some("assets/saved.png".into())
}

#[gpui::test]
fn handler_can_use_the_installed_store_during_the_editor_update(cx: &mut TestAppContext) {
    let (editor, mut visual) = open("", Mode::Source, cx);
    visual.update(|_, cx| {
        cx.set_global(Destination {
            editor: editor.entity_id(),
            mode: Mode::Source,
            in_fence: true,
            calls: 0,
        });
        cx.set_global(Kept(0));
        cx.set_image_store(ImageStore {
            keep,
            ..ImageStore::default()
        });
        cx.set_paste_handler(|item, editor, destination, cx| {
            let input = match &item.entries()[0] {
                ClipboardEntry::ExternalPaths(paths) => Source::File(&paths.paths()[0]),
                ClipboardEntry::Image(image) => Source::Bytes(image),
                _ => return None,
            };
            let store = cx.image_store();
            let url = (store.keep)(input, editor, destination.base, cx)?;
            cx.global_mut::<Kept>().0 += 1;
            Some(PasteContent::Literal(format!("![]({url})")))
        });
    });
    for entry in [
        ClipboardEntry::ExternalPaths(ExternalPaths(
            vec![PathBuf::from("/clipboard/a.png")].into(),
        )),
        ClipboardEntry::Image(gpui::Image::from_bytes(
            gpui::ImageFormat::Png,
            vec![1, 2, 3],
        )),
    ] {
        paste(
            ClipboardItem {
                entries: vec![entry],
            },
            &mut visual,
        );
    }
    assert_eq!(
        source(&editor, &mut visual),
        "![](assets/saved.png)![](assets/saved.png)"
    );
    assert_eq!(visual.update(|_, cx| cx.global::<Kept>().0), 2);
}

#[gpui::test]
fn declining_the_handler_preserves_media_priority_and_source_fallback(cx: &mut TestAppContext) {
    for (mode, expected) in [
        (Mode::Blocks, "![](/clipboard/a.png)"),
        (Mode::Source, "path text"),
    ] {
        let (editor, mut visual) = open("", mode, cx);
        visual.update(|_, cx| cx.set_paste_handler(|_, _, _, _| None));
        paste(
            ClipboardItem {
                entries: vec![
                    ClipboardEntry::ExternalPaths(ExternalPaths(
                        vec![PathBuf::from("/clipboard/a.png")].into(),
                    )),
                    ClipboardEntry::String(ClipboardString::new("path text".into())),
                ],
            },
            &mut visual,
        );
        assert_eq!(source(&editor, &mut visual), expected);
    }
}
