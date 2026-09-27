use gpui::{EntityInputHandler, TestAppContext};
use ui::input::TextField;

#[gpui::test]
fn composition_selection_is_relative_to_replacement_text(cx: &mut TestAppContext) {
    cx.update(|cx| theme::Theme::install(theme::Appearance::Dark, cx));
    let window = cx.add_window(|_, cx| TextField::new(cx));
    window
        .update(cx, |field, window, cx| {
            field.set_content("中abc!", cx);
            // Replace ASCII after a Chinese prefix; select the emoji inside
            // the replacement. The old range has a different length.
            field.replace_and_mark_text_in_range(Some(1..4), "😀文", Some(0..2), window, cx);
            assert_eq!(field.content().as_ref(), "中😀文!");
            assert_eq!(field.marked_text_range(window, cx), Some(1..4));
            assert_eq!(
                field.selected_text_range(false, window, cx).unwrap().range,
                1..3
            );
            // Update the marked range, with no explicit replacement range.
            field.replace_and_mark_text_in_range(None, "中文", Some(1..1), window, cx);
            assert_eq!(field.content().as_ref(), "中中文!");
            assert_eq!(
                field.selected_text_range(false, window, cx).unwrap().range,
                2..2
            );
            field.replace_text_in_range(None, "文", window, cx);
            assert_eq!(field.content().as_ref(), "中文!");
            assert_eq!(field.marked_text_range(window, cx), None);
        })
        .unwrap();
}

/// A range handed to `select` is floored onto char boundaries and the end of
/// the text, with the caret at its end.
#[gpui::test]
fn select_clamps_to_char_boundaries(cx: &mut TestAppContext) {
    cx.update(|cx| theme::Theme::install(theme::Appearance::Dark, cx));
    let window = cx.add_window(|_, cx| TextField::new(cx));
    window
        .update(cx, |field, window, cx| {
            field.set_content("a中b", cx);
            // Byte 2 is inside `中` (bytes 1..4); 99 is past the end.
            field.select(2..99, cx);
            assert_eq!(
                field.selected_text_range(false, window, cx).unwrap().range,
                1..3,
                "utf-16: `中` starts at 1, the end is 3"
            );
            assert_eq!(field.cursor(), 5);
        })
        .unwrap();
}
