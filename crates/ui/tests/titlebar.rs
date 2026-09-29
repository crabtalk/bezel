use gpui::{Context, Render, TestAppContext, VisualTestContext, Window, div, prelude::*, px, size};
use theme::{Appearance, Theme};
use ui::{
    AppExt as _,
    titlebar::{self, CaptionSide, CaptionStyle},
};

#[gpui::test]
fn caption_style_defaults_to_rectangular_and_can_be_reset(cx: &mut TestAppContext) {
    cx.update(|cx| {
        assert_eq!(cx.caption_style(), CaptionStyle::Rectangular);
        cx.set_caption_style(CaptionStyle::Lights);
        assert_eq!(cx.caption_style(), CaptionStyle::Lights);
        cx.set_caption_style(CaptionStyle::Rectangular);
        assert_eq!(cx.caption_style(), CaptionStyle::Rectangular);
    });
}

struct Host;

impl Render for Host {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        titlebar::titlebar("test-titlebar", false, window)
            .child(titlebar::controls(CaptionSide::Left, window, cx))
            .child(div().flex_1())
            .child(titlebar::controls(CaptionSide::Right, window, cx))
    }
}

#[gpui::test]
fn styles_keep_full_height_targets_order_and_outer_corner(cx: &mut TestAppContext) {
    cx.update(|cx| Theme::install(Appearance::Dark, cx));
    let window = cx.add_window(|_, _| Host);
    let mut cx = VisualTestContext::from_window(window.into(), cx);
    cx.simulate_resize(size(px(400.0), px(200.0)));

    for style in [CaptionStyle::Rectangular, CaptionStyle::Lights] {
        cx.update(|_, cx| cx.set_caption_style(style));
        cx.run_until_parked();
        let mut right = px(400.0);
        for selector in ["caption-close", "caption-maximize", "caption-minimize"] {
            let bounds = cx.debug_bounds(selector).expect("caption target");
            assert_eq!(
                bounds.size,
                size(px(Theme::CAPTION_BUTTON_WIDTH), px(Theme::TITLEBAR_HEIGHT))
            );
            assert_eq!(bounds.origin.y, px(0.0));
            assert_eq!(bounds.right(), right);
            right = bounds.origin.x;
        }
    }
}

#[gpui::test]
fn both_styles_hide_all_controls_in_fullscreen(cx: &mut TestAppContext) {
    cx.update(|cx| Theme::install(Appearance::Dark, cx));
    let window = cx.add_window(|_, _| Host);
    let mut cx = VisualTestContext::from_window(window.into(), cx);
    cx.update(|window, _| window.toggle_fullscreen());
    for style in [CaptionStyle::Rectangular, CaptionStyle::Lights] {
        cx.update(|_, cx| cx.set_caption_style(style));
        cx.run_until_parked();
        for selector in ["caption-close", "caption-maximize", "caption-minimize"] {
            assert!(cx.debug_bounds(selector).is_none());
        }
    }
}
