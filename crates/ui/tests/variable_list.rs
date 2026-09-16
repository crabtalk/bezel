use gpui::{Context, Render, Window, div, prelude::*, px};
use std::{cell::Cell, rc::Rc};
use ui::list::VariableList;
struct Feed {
    extra: Rc<Cell<f32>>,
    list: VariableList<usize>,
    rendered: Rc<Cell<usize>>,
}
impl Render for Feed {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let rendered = self.rendered.clone();
        let extra = self.extra.clone();
        div().w(px(400.)).h(px(300.)).child(self.list.render(
            move |ix, _, _| {
                rendered.set(rendered.get() + 1);
                div()
                    .h(px(40.
                        + (ix % 3) as f32 * 10.
                        + if ix == 50 { extra.get() } else { 0. }))
                    .child(format!("Row {ix}"))
                    .into_any_element()
            },
            |_, _, _| {},
        ))
    }
}
#[gpui::test]
fn huge_history_builds_only_nearby_rows_and_retains_anchor(cx: &mut gpui::TestAppContext) {
    cx.update(|cx| theme::Theme::install(theme::Appearance::Light, cx));
    let list = VariableList::default();
    list.sync((10..10010).collect());
    let rendered = Rc::new(Cell::new(0));
    let window = cx.add_window(|_, _| Feed {
        extra: Default::default(),
        list: list.clone(),
        rendered: rendered.clone(),
    });
    let mut visual = gpui::VisualTestContext::from_window(window.into(), cx);
    visual.run_until_parked();
    assert!(rendered.get() < 100, "built {} rows", rendered.get());
    list.scroll_to(500);
    visual.update(|window, _| window.refresh());
    visual.run_until_parked();
    let before = list.state.logical_scroll_top();
    list.sync((0..10010).collect());
    visual.update(|window, _| window.refresh());
    visual.run_until_parked();
    let after = list.state.logical_scroll_top();
    assert_eq!(after.item_ix, before.item_ix + 10);
    assert_eq!(after.offset_in_item, before.offset_in_item);
    assert!(!list.state.is_following_tail());
    assert!(list.visible_range().contains(&after.item_ix));
    list.invalidate(&510);
    visual.update(|window, _| window.refresh());
    visual.run_until_parked();
    assert_eq!(list.state.logical_scroll_top().item_ix, after.item_ix);
}
#[gpui::test]
fn appending_keeps_tail_following(cx: &mut gpui::TestAppContext) {
    cx.update(|cx| theme::Theme::install(theme::Appearance::Light, cx));
    let list = VariableList::default();
    list.sync((0..100).collect());
    let window = cx.add_window(|_, _| Feed {
        extra: Default::default(),
        list: list.clone(),
        rendered: Default::default(),
    });
    let mut visual = gpui::VisualTestContext::from_window(window.into(), cx);
    visual.run_until_parked();
    list.sync((0..101).collect());
    list.invalidate(&100);
    visual.update(|window, _| window.refresh());
    visual.run_until_parked();
    assert!(list.state.is_following_tail());
    assert_eq!(list.visible_range().end, 101);
}

#[gpui::test]
fn remeasurement_preserves_offset_inside_the_anchor(cx: &mut gpui::TestAppContext) {
    cx.update(|cx| theme::Theme::install(theme::Appearance::Light, cx));
    let list = VariableList::default();
    list.sync((0..100).collect());
    let window = cx.add_window(|_, _| Feed {
        extra: Default::default(),
        list: list.clone(),
        rendered: Default::default(),
    });
    let mut visual = gpui::VisualTestContext::from_window(window.into(), cx);
    visual.run_until_parked();
    list.state.pause_following_tail();
    list.state.scroll_to(gpui::ListOffset {
        item_ix: 50,
        offset_in_item: px(17.),
    });
    visual.update(|window, _| window.refresh());
    visual.run_until_parked();
    window
        .update(&mut visual, |feed, _, _| feed.extra.set(200.))
        .unwrap();
    list.invalidate(&50);
    visual.update(|window, _| window.refresh());
    visual.run_until_parked();
    assert_eq!(list.state.logical_scroll_top().item_ix, 50);
    assert_eq!(list.state.logical_scroll_top().offset_in_item, px(17.));
}
