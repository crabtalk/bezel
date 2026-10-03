use gpui::{
    Context, Entity, Focusable, Render, TestAppContext, VisualTestContext, Window, div, prelude::*,
    px,
};
use theme::{Appearance, Theme};
use ui::{
    AppExt as _,
    input::{CaretHeight, CaretShape, TextField},
};

struct Page(Entity<TextField>);

impl Render for Page {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().w(px(100.0)).child(self.0.clone())
    }
}

fn caret(cx: &mut VisualTestContext, shape: CaretShape) -> gpui::Quad {
    cx.update(|_, cx| cx.set_caret_shape(shape));
    cx.run_until_parked();
    cx.update(|window, cx| {
        let color = Theme::of(cx).caret;
        // Solid in the caret's own colour; a block in an inactive window is
        // its outline.
        window
            .painted_quads()
            .into_iter()
            .find(|quad| {
                quad.background == color.into()
                    || (shape == CaretShape::Block && quad.border_color == color)
            })
            .expect("caret is painted")
    })
}

#[gpui::test]
fn field_shapes_follow_character_width_and_stay_inside_the_clip(cx: &mut TestAppContext) {
    cx.update(|cx| {
        Theme::install(Appearance::Dark, cx);
        assert_eq!(cx.caret_shape(), CaretShape::Bar);
        cx.set_caret_blink(false);
    });
    let window = cx.add_window(|_, cx| Page(cx.new(|cx| TextField::new(cx).with_frame(false))));
    let page = window.root(cx).unwrap();
    let field = cx.update(|cx| page.read(cx).0.clone());
    let mut visual = VisualTestContext::from_window(window.into(), cx);
    visual.update(|window, cx| window.focus(&field.read(cx).focus_handle(cx), cx));
    // Where no character follows the caret it takes the font's `ch` width:
    // what it measures over a `0`.
    visual.update(|_, cx| {
        field.update(cx, |field, cx| {
            field.set_content("0", cx);
            field.select(0..0, cx);
        })
    });
    let ch = caret(&mut visual, CaretShape::Block).bounds.size.width;
    for text in ["i", "W", "中", "e\u{301}", "", "a long line ending in W"] {
        visual.update(|_, cx| {
            field.update(cx, |field, cx| {
                field.set_content(text, cx);
                let offset = text.rfind('W').unwrap_or(0);
                field.select(offset..offset, cx);
            })
        });
        let bar = caret(&mut visual, CaretShape::Bar);
        let block = caret(&mut visual, CaretShape::Block);
        let underline = caret(&mut visual, CaretShape::Underline);
        assert_eq!(
            bar.bounds.size.width,
            gpui::ScaledPixels(2.0 * visual.update(|window, _| window.scale_factor()))
        );
        // A block fills the line; the bar is the font's height inside it.
        assert!(block.bounds.size.height > bar.bounds.size.height);
        assert!((block.bounds.center().y.0 - bar.bounds.center().y.0).abs() <= 1.0);
        assert!(block.bounds.size.width > bar.bounds.size.width);
        assert_eq!(underline.bounds.size.width, block.bounds.size.width);
        assert_eq!(
            underline.bounds.size.height,
            gpui::ScaledPixels(2.0 * visual.update(|window, _| window.scale_factor()))
        );
        assert_eq!(underline.bounds.bottom(), bar.bounds.bottom());
        assert!(block.bounds.right() <= block.content_mask.bounds.right());

        visual
            .update(|_, cx| field.update(cx, |field, cx| field.select(text.len()..text.len(), cx)));
        let end = caret(&mut visual, CaretShape::Block);
        // Painted quads snap to device pixels, so the two can round apart.
        assert!((end.bounds.size.width.0 - ch.0).abs() <= 1.0);
        assert!(end.bounds.right() <= end.content_mask.bounds.right());
    }
}

#[gpui::test]
fn advances_use_shaped_graphemes_and_stop_at_wraps(cx: &mut TestAppContext) {
    cx.update(|cx| Theme::install(Appearance::Light, cx));
    let window = cx.add_window(|_, cx| TextField::new(cx));
    window
        .update(cx, |_, window, _| {
            let style = window.text_style();
            for text in ["iW", "中文", "e\u{301}x", "🇨🇳x"] {
                let lines = window
                    .text_system()
                    .shape_text(
                        text.into(),
                        px(16.0),
                        &[gpui::TextRun {
                            len: text.len(),
                            font: style.font(),
                            color: style.color,
                            background_color: None,
                            underline: None,
                            strikethrough: None,
                        }],
                        None,
                        None,
                    )
                    .unwrap();
                let line = &lines[0];
                let next = ui::input::next_boundary(text, 0);
                let advance = ui::input::caret::character_advance(line, text, 0).unwrap();
                assert_eq!(
                    advance,
                    line.unwrapped_layout.x_for_index(next) - line.unwrapped_layout.x_for_index(0)
                );
                assert!(advance > px(0.0));
                assert_eq!(
                    ui::input::caret::character_advance(line, text, text.len()),
                    None
                );
            }
            let text = "wide words wrap here";
            let lines = window
                .text_system()
                .shape_text(
                    text.into(),
                    px(16.0),
                    &[gpui::TextRun {
                        len: text.len(),
                        font: style.font(),
                        color: style.color,
                        background_color: None,
                        underline: None,
                        strikethrough: None,
                    }],
                    Some(px(40.0)),
                    None,
                )
                .unwrap();
            let line = &lines[0];
            assert!(!line.wrap_boundaries().is_empty());
            for boundary in line.wrap_boundaries() {
                let offset = line.runs()[boundary.run_ix].glyphs[boundary.glyph_ix].index;
                assert_eq!(
                    ui::input::caret::character_advance(line, text, offset),
                    None
                );
            }
        })
        .unwrap();
}

#[gpui::test]
fn block_is_solid_while_active_and_hollow_while_not(cx: &mut TestAppContext) {
    cx.update(|cx| {
        Theme::install(Appearance::Dark, cx);
        cx.set_caret_blink(false);
    });
    let window = cx.add_window(|_, cx| Page(cx.new(|cx| TextField::new(cx).with_frame(false))));
    let page = window.root(cx).unwrap();
    let field = cx.update(|cx| page.read(cx).0.clone());
    let mut visual = VisualTestContext::from_window(window.into(), cx);
    visual.update(|window, cx| {
        window.activate_window();
        window.focus(&field.read(cx).focus_handle(cx), cx);
        field.update(cx, |field, cx| field.set_content("ab", cx));
    });
    let color = visual.update(|_, cx| Theme::of(cx).caret);
    assert_eq!(color.a, 1.0);
    let solid = caret(&mut visual, CaretShape::Block);
    assert_eq!(solid.background, color.into());

    visual.deactivate_window();
    let hollow = caret(&mut visual, CaretShape::Block);
    assert_eq!(hollow.border_color, color);
    assert_ne!(hollow.background, color.into());
    assert_eq!(hollow.bounds, solid.bounds);
}

#[test]
fn only_a_solid_block_cuts_its_glyph_out() {
    assert!(CaretShape::Block.cuts_out(false));
    assert!(!CaretShape::Block.cuts_out(true));
    assert!(!CaretShape::Bar.cuts_out(false));
    assert!(!CaretShape::Underline.cuts_out(false));
}

#[test]
fn recoloured_cuts_runs_at_the_range() {
    let run = |len, color| gpui::TextRun {
        len,
        font: gpui::font("Helvetica"),
        color,
        background_color: None,
        underline: None,
        strikethrough: None,
    };
    let (red, blue, bg) = (gpui::red(), gpui::blue(), gpui::black());
    let runs = ui::input::caret::recoloured(vec![run(3, red), run(3, blue)], &(2..4), bg);
    let pieces: Vec<_> = runs.iter().map(|run| (run.len, run.color)).collect();
    assert_eq!(pieces, [(2, red), (1, bg), (1, bg), (2, blue)]);
}

#[gpui::test]
fn a_text_height_block_stands_where_the_bar_does(cx: &mut TestAppContext) {
    cx.update(|cx| {
        Theme::install(Appearance::Dark, cx);
        assert_eq!(cx.caret_height(), CaretHeight::Line);
        cx.set_caret_blink(false);
    });
    let window = cx.add_window(|_, cx| Page(cx.new(|cx| TextField::new(cx).with_frame(false))));
    let page = window.root(cx).unwrap();
    let field = cx.update(|cx| page.read(cx).0.clone());
    let mut visual = VisualTestContext::from_window(window.into(), cx);
    visual.update(|window, cx| window.focus(&field.read(cx).focus_handle(cx), cx));
    visual.update(|_, cx| {
        field.update(cx, |field, cx| {
            field.set_content("W", cx);
            field.select(0..0, cx);
        })
    });
    let line = caret(&mut visual, CaretShape::Block);
    visual.update(|_, cx| cx.set_caret_height(CaretHeight::Text));
    let bar = caret(&mut visual, CaretShape::Bar);
    let text = caret(&mut visual, CaretShape::Block);
    assert!(text.bounds.size.height < line.bounds.size.height);
    assert_eq!(text.bounds.origin.y, bar.bounds.origin.y);
    assert_eq!(text.bounds.size.height, bar.bounds.size.height);
    assert_eq!(text.bounds.size.width, line.bounds.size.width);
}
