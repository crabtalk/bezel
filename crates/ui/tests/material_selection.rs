use gpui::{
    Bounds, Context, Modifiers, MouseButton, Render, TestAppContext, VisualTestContext, Window,
    div, point, prelude::*, px, size,
};
use theme::Theme;
use ui::{surface, tabs};

struct Controls {
    clicks: Vec<usize>,
}

impl Render for Controls {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        window.native_surfaces.materials = true;
        let theme = Theme::of(cx);
        div().flex().children((0..2).map(|index| {
            tabs::chip(theme, tabs::State::Focused)
                .id(index)
                .w(px(100.))
                .debug_selector(move || format!("selection-{index}"))
                .child("Tab")
                .on_click(cx.listener(move |host, _, _, _| host.clicks.push(index)))
        }))
    }
}

#[gpui::test]
fn native_selections_preserve_control_geometry_and_independent_clicks(cx: &mut TestAppContext) {
    cx.update(|cx| {
        Theme::install_custom(
            Theme {
                glass: true,
                ..Theme::dark()
            },
            cx,
        )
    });
    let window = cx.add_window(|_, _| Controls { clicks: Vec::new() });
    let host = window.root(cx).unwrap();
    let mut visual = VisualTestContext::from_window(window.into(), cx);
    visual.simulate_resize(size(px(300.), px(80.)));
    visual.run_until_parked();
    for selector in ["selection-0", "selection-1"] {
        let bounds = visual.debug_bounds(selector).unwrap();
        assert_eq!(bounds.size, size(px(100.), px(Theme::BUTTON_HEIGHT)));
        visual.simulate_mouse_down(bounds.center(), MouseButton::Left, Modifiers::default());
        visual.simulate_mouse_up(bounds.center(), MouseButton::Left, Modifiers::default());
        visual.run_until_parked();
    }
    visual.update(|_, cx| assert_eq!(host.read(cx).clicks, [0, 1]));
}

struct MovingSelection;

impl Render for MovingSelection {
    fn render(&mut self, window: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        window.native_surfaces.materials = true;
        surface::selection(
            "moving-selection",
            div().w(px(200.)).h(px(30.)).bg(gpui::rgb(0x123456)),
            |bounds, _, _| {
                Some((
                    Bounds::new(
                        bounds.origin + point(px(80.), px(0.)),
                        size(px(60.), px(30.)),
                    ),
                    px(15.),
                ))
            },
        )
    }
}

#[gpui::test]
fn moving_material_keeps_the_foreground_viewport(cx: &mut TestAppContext) {
    cx.update(|cx| {
        Theme::install_custom(
            Theme {
                glass: true,
                ..Theme::dark()
            },
            cx,
        )
    });
    let window = cx.add_window(|_, _| MovingSelection);
    let mut visual = VisualTestContext::from_window(window.into(), cx);
    visual.simulate_resize(size(px(300.), px(80.)));
    visual.run_until_parked();
    visual.update(|window, _| {
        let scale = window.scale_factor();
        let quads = window.painted_quads();
        let foreground = quads
            .iter()
            .find(|quad| quad.bounds.size.width.0 == 200. * scale)
            .unwrap();
        assert_eq!(foreground.content_mask.bounds.size.width.0, 200. * scale);
        assert_eq!(foreground.content_mask.bounds.size.height.0, 30. * scale);
        let scene = window.rendered_scene();
        assert_eq!(scene.native_materials.len(), 1);
        assert!(
            foreground.order > scene.native_materials[0].end_order,
            "the moving highlight must not own the wider foreground"
        );
    });
}
