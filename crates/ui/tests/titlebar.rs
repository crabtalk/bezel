use gpui::{
    Context, Render, TestAppContext, VisualTestContext, Window, WindowButton, WindowButtonLayout,
    WindowControls, div, prelude::*, px, size,
};
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
        assert_eq!(
            cx.update(|window, _| titlebar::lights_width(window)),
            titlebar::LIGHTS_WIDTH
        );
        let (mut left, selectors) = match style {
            CaptionStyle::Rectangular => (
                px(400.0 - titlebar::LIGHTS_WIDTH),
                ["caption-minimize", "caption-maximize", "caption-close"],
            ),
            CaptionStyle::Lights => (
                px(0.0),
                ["caption-close", "caption-minimize", "caption-maximize"],
            ),
        };
        for selector in selectors {
            let bounds = cx.debug_bounds(selector).expect("caption target");
            assert_eq!(
                bounds.size,
                size(px(Theme::CAPTION_BUTTON_WIDTH), px(Theme::TITLEBAR_HEIGHT))
            );
            assert_eq!(bounds.origin.y, px(0.0));
            assert_eq!(bounds.origin.x, left);
            left = bounds.right();
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
        assert_eq!(cx.update(|window, _| titlebar::lights_width(window)), 0.0);
        for selector in ["caption-close", "caption-maximize", "caption-minimize"] {
            assert!(cx.debug_bounds(selector).is_none());
        }
    }
}

#[test]
fn lights_ignore_desktop_layout_and_filter_permissions() {
    let desktop = WindowButtonLayout {
        left: [Some(WindowButton::Maximize), None, None],
        right: [
            Some(WindowButton::Minimize),
            Some(WindowButton::Close),
            None,
        ],
    };
    for minimize in [false, true] {
        for maximize in [false, true] {
            let allowed = WindowControls {
                minimize,
                maximize,
                ..WindowControls::default()
            };
            for desktop in [None, Some(desktop)] {
                let layout = CaptionStyle::Lights.button_layout(desktop, allowed);
                assert_eq!(
                    layout.left,
                    [
                        Some(WindowButton::Close),
                        minimize.then_some(WindowButton::Minimize),
                        maximize.then_some(WindowButton::Maximize),
                    ]
                );
                assert_eq!(layout.right, [None; 3]);
            }
            let layout = CaptionStyle::Rectangular.button_layout(Some(desktop), allowed);
            assert_eq!(
                layout.left,
                [maximize.then_some(WindowButton::Maximize), None, None]
            );
            assert_eq!(
                layout.right,
                [
                    minimize.then_some(WindowButton::Minimize),
                    Some(WindowButton::Close),
                    None
                ]
            );
        }
    }
}
