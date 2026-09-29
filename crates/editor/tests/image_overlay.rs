use std::{cell::Cell, rc::Rc};

use editor::{Editor, Mode};
use gpui::{Modifiers, TestAppContext, VisualTestContext, div, prelude::*, px};
use markdown::ImageOverlayCorner;

const PIXEL: &[u8] = &[
    137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 1, 0, 0, 0, 1, 8, 6, 0,
    0, 0, 31, 21, 196, 137, 0, 0, 0, 13, 73, 68, 65, 84, 120, 218, 99, 100, 96, 248, 95, 15, 0, 2,
    135, 1, 128, 235, 71, 186, 146, 0, 0, 0, 0, 73, 69, 78, 68, 174, 66, 96, 130,
];

#[gpui::test]
fn editor_forwards_picture_controls_and_can_replace_move_and_remove_them(cx: &mut TestAppContext) {
    let path =
        std::env::temp_dir().join(format!("bezel-editor-overlay-{}.png", std::process::id()));
    std::fs::write(&path, PIXEL).unwrap();
    let source = format!("before\n\n![caption|200]({})\n\nafter", path.display());
    let clicked = Rc::new(Cell::new(0));
    let clicks = clicked.clone();
    let overlay: markdown::ImageOverlay = Rc::new(move |block, url, _, _| {
        assert_eq!(block, 1);
        assert!(url.ends_with(".png"));
        let clicks = clicks.clone();
        Some(
            div()
                .id("article-image-control")
                .debug_selector(|| "article-image-control".into())
                .size(px(24.0))
                .on_click(move |_, _, _| clicks.set(clicks.get() + 1))
                .into_any_element(),
        )
    });
    cx.update(|cx| {
        theme::Theme::install(theme::Appearance::Dark, cx);
        editor::init(cx);
    });
    let window = cx.add_window(|_, cx| {
        Editor::new(&source, cx)
            .with_image_overlay(overlay.clone())
            .with_image_overlay_corner(ImageOverlayCorner::TopRight)
    });
    let editor = window.root(cx).unwrap();
    let mut visual = VisualTestContext::from_window(window.into(), cx);
    let settle = |visual: &mut VisualTestContext| {
        for _ in 0..4 {
            visual.update(|window, _| window.refresh());
            visual.run_until_parked();
        }
    };
    settle(&mut visual);
    let picture = visual.update(|_, cx| editor.read(cx).layouts().picture_bounds(1).unwrap());
    visual.simulate_mouse_move(picture.center(), None, Modifiers::default());
    settle(&mut visual);
    let control = visual
        .debug_bounds("article-image-control")
        .expect("editor forwards overlay");
    assert_eq!(control.top(), picture.top() + px(6.0));
    let selection = visual.update(|_, cx| editor.read(cx).selection());
    visual.simulate_click(control.center(), Modifiers::default());
    assert_eq!(clicked.get(), 1);
    assert_eq!(
        visual.update(|_, cx| editor.read(cx).selection()),
        selection
    );
    assert_eq!(visual.update(|_, cx| editor.read(cx).source()), source);
    visual.update(|_, cx| {
        editor.update(cx, |editor, cx| {
            editor.set_image_overlay_corner(ImageOverlayCorner::BottomRight, cx)
        })
    });
    settle(&mut visual);
    let control = visual.debug_bounds("article-image-control").unwrap();
    assert_eq!(control.bottom(), picture.bottom() - px(6.0));
    visual.update(|_, cx| editor.update(cx, |editor, cx| editor.set_mode(Mode::Source, cx)));
    settle(&mut visual);
    assert!(visual.debug_bounds("article-image-control").is_none());
    visual.update(|_, cx| editor.update(cx, |editor, cx| editor.set_mode(Mode::Blocks, cx)));
    settle(&mut visual);
    assert!(visual.debug_bounds("article-image-control").is_some());
    visual.update(|_, cx| editor.update(cx, |editor, cx| editor.set_image_overlay(None, cx)));
    settle(&mut visual);
    assert!(visual.debug_bounds("article-image-control").is_none());
    visual.update(|_, cx| {
        editor.update(cx, |editor, cx| editor.set_image_overlay(Some(overlay), cx))
    });
    settle(&mut visual);
    assert!(visual.debug_bounds("article-image-control").is_some());
    std::fs::remove_file(path).unwrap();
}
