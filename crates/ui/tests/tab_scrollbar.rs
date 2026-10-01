use gpui::{
    Context, Modifiers, MouseButton, Render, TestAppContext, VisualTestContext, Window, div, point,
    prelude::*, px, size,
};
use ui::{AppExt as _, scroll::Visibility, tabs};

struct Host {
    strip: tabs::Strip<&'static str>,
    reorder: tabs::Reorder<&'static str>,
    sortable: bool,
    changes: usize,
    outside: Vec<tabs::OutsideDrop<&'static str>>,
}

impl Render for Host {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = theme::Theme::of(cx);
        let tabs: Vec<_> = self
            .strip
            .tabs()
            .iter()
            .map(|&id| {
                (
                    id,
                    tabs::tab(theme, id, tabs::Label::new(id), tabs::State::Resting)
                        .w(px(100.))
                        .debug_selector(move || id.into()),
                )
            })
            .collect();
        let bar = if self.sortable {
            self.reorder
                .bar("strip", &self.strip, tabs)
                .on_reorder(cx.listener(|view, event: &tabs::Move, _, cx| {
                    view.strip.reorder(event.from, event.to);
                    view.changes += 1;
                    cx.notify();
                }))
                .on_drop_outside(cx.listener(
                    |view, event: &tabs::OutsideDrop<&'static str>, _, _| {
                        view.outside.push(event.clone())
                    },
                ))
                .into_any_element()
        } else {
            tabs::bar("strip")
                .children(tabs.into_iter().map(|(_, tab)| tab))
                .into_any_element()
        };
        div().w_full().child(bar)
    }
}

fn open(sortable: bool, cx: &mut TestAppContext) -> (gpui::Entity<Host>, VisualTestContext) {
    cx.update(|cx| {
        theme::Theme::install(theme::Appearance::Dark, cx);
        cx.set_scrollbar_visibility(Visibility::Always);
    });
    let window = cx.add_window(|_, cx| Host {
        strip: ["a", "b", "c"].into_iter().collect(),
        reorder: tabs::Reorder::new(motion::Painter::of(cx)),
        sortable,
        changes: 0,
        outside: Vec::new(),
    });
    let view = window.root(cx).unwrap();
    let mut visual = VisualTestContext::from_window(window.into(), cx);
    visual.simulate_resize(size(px(180.), px(140.)));
    visual.run_until_parked();
    visual.update(|window, _| window.refresh());
    visual.run_until_parked();
    (view, visual)
}

fn drag_thumb(cx: &mut VisualTestContext) {
    let thumb = cx
        .debug_bounds("tab-scrollbar-thumb")
        .expect("overflowing strip has a thumb");
    let from = thumb.center();
    cx.simulate_mouse_move(from, None, Modifiers::default());
    cx.simulate_mouse_down(from, MouseButton::Left, Modifiers::default());
    cx.simulate_mouse_move(
        from + point(px(5.), px(0.)),
        MouseButton::Left,
        Modifiers::default(),
    );
    let to = from + point(px(40.), px(0.));
    cx.simulate_mouse_move(to, MouseButton::Left, Modifiers::default());
    cx.simulate_mouse_up(to, MouseButton::Left, Modifiers::default());
    cx.run_until_parked();
}

#[gpui::test]
fn plain_bar_owns_a_persistent_overlay_without_changing_tab_height(cx: &mut TestAppContext) {
    let (view, mut cx) = open(false, cx);
    let before = cx.debug_bounds("a").unwrap();
    drag_thumb(&mut cx);
    let scrolled = cx.debug_bounds("a").unwrap();
    assert!(scrolled.left() < before.left());
    assert_eq!(scrolled.size.height, before.size.height);
    view.update(&mut cx, |_, cx| cx.notify());
    cx.run_until_parked();
    assert_eq!(cx.debug_bounds("a").unwrap(), scrolled);
    cx.simulate_resize(size(px(500.), px(140.)));
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
    assert!(cx.debug_bounds("tab-scrollbar-thumb").is_none());
}

#[gpui::test]
fn reorder_bar_scrolls_without_reordering_and_forwards_outside_coordinates(
    cx: &mut TestAppContext,
) {
    let (view, mut cx) = open(true, cx);
    drag_thumb(&mut cx);
    assert!(cx.debug_bounds("a").unwrap().left() < px(0.));
    assert_eq!(cx.update(|_, cx| view.read(cx).changes), 0);
    let source = cx.debug_bounds("b").unwrap().center();
    cx.simulate_mouse_down(source, MouseButton::Left, Modifiers::default());
    let end = point(px(130.), px(100.));
    cx.simulate_mouse_move(end, MouseButton::Left, Modifiers::default());
    cx.simulate_mouse_up(end, MouseButton::Left, Modifiers::default());
    cx.run_until_parked();
    cx.update(|_, cx| {
        let view = view.read(cx);
        assert_eq!(view.outside.len(), 1);
        assert_eq!(view.outside[0].id, "b");
        assert_eq!(view.outside[0].position, end);
    });
}

#[gpui::test]
fn overlay_respects_the_app_visibility_setting(cx: &mut TestAppContext) {
    let (_, mut cx) = open(true, cx);
    assert!(cx.debug_bounds("tab-scrollbar-thumb").is_some());
    cx.update(|_, cx| cx.set_scrollbar_visibility(Visibility::Never));
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
    assert!(cx.debug_bounds("tab-scrollbar-thumb").is_none());
}
