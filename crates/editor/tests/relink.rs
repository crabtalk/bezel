use editor::{Editor, Mode};
use gpui::{Entity, Focusable, TestAppContext, VisualTestContext};
use markdown::{BlockKind, Cursor, Part, Selection};

const WEB: &str = "https://example.com/p.png";
const LOCAL: &str = "assets/media-1.png";

fn open(source: &str, mode: Mode, cx: &mut TestAppContext) -> (Entity<Editor>, VisualTestContext) {
    cx.update(|cx| {
        theme::Theme::install(theme::Appearance::Dark, cx);
        editor::init(cx);
    });
    let window = cx.add_window(|_, cx| Editor::new(source, cx).with_mode(mode));
    let editor = window.root(cx).unwrap();
    let mut visual = VisualTestContext::from_window(window.into(), cx);
    visual.update(|window, cx| window.focus(&editor.read(cx).focus_handle(cx), cx));
    visual.run_until_parked();
    (editor, visual)
}

#[gpui::test]
fn relink_rewrites_each_picture_in_source(cx: &mut TestAppContext) {
    let text = format!("a ![]({WEB}) b ![x](<{WEB}>) end");
    let (editor, mut cx) = open("", Mode::Source, cx);
    cx.simulate_input(&text);
    let before = cx.update(|_, cx| editor.read(cx).source());
    let end = Cursor::new(0, Part::Code, before.len());
    cx.update(|_, cx| {
        editor.update(cx, |editor, cx| {
            editor.select(Selection::at(end), cx);
            editor.relink(WEB, LOCAL, cx);
        })
    });

    let source = cx.update(|_, cx| editor.read(cx).source());
    assert!(before.contains(&format!("](<{WEB}>)")), "{before:?}");
    assert_eq!(source, before.replace(WEB, LOCAL));
    cx.simulate_input("!");
    assert!(
        cx.update(|_, cx| editor.read(cx).source())
            .ends_with("end!")
    );
}

#[gpui::test]
fn relink_points_image_blocks(cx: &mut TestAppContext) {
    let (editor, mut cx) = open(&format!("![alt]({WEB})\n\ntext"), Mode::Blocks, cx);
    cx.update(|_, cx| editor.update(cx, |editor, cx| editor.relink(WEB, LOCAL, cx)));

    let kind = cx.update(|_, cx| editor.read(cx).doc().blocks[0].kind.clone());
    assert!(matches!(kind, BlockKind::Image { url, .. } if url == LOCAL));
}
