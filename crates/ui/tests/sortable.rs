use std::time::Duration;

use gpui::{
    Axis, Context, Entity, Modifiers, MouseButton, Pixels, Point, Render, ScrollHandle,
    TestAppContext, VisualTestContext, Window, div, point, prelude::*, px, size,
};
use ui::sortable::{self, Sortable};

struct Board {
    sortable: Sortable<usize, &'static str>,
    lists: Vec<Vec<&'static str>>,
    kinds: [&'static str; 3],
    outside: Vec<sortable::OutsideDrop<&'static str>>,
    moves: Vec<sortable::Move<usize, &'static str>>,
    scrolls: Vec<ScrollHandle>,
    parent_scroll: ScrollHandle,
    clicks: usize,
    axis: Axis,
    clipped: bool,
}

impl Render for Board {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let lists = self.lists.iter().enumerate().map(|(list, items)| {
            let items = items.iter().map(|&id| {
                let extent = match id {
                    "b" => 80.,
                    "d" => 60.,
                    _ => 40.,
                };
                (
                    id,
                    div()
                        .id(id)
                        .debug_selector(move || id.into())
                        .flex_none()
                        .when(self.axis == Axis::Vertical, |el| el.h(px(extent)).w_full())
                        .when(self.axis == Axis::Horizontal, |el| {
                            el.w(px(extent)).h(px(40.))
                        })
                        .on_click(cx.listener(|view, _, _, _| view.clicks += 1)),
                )
            });
            sortable::List::new(("list", list), list, self.axis, items)
                .kind(self.kinds[list])
                .gap(px(4.))
                .track_scroll(&self.scrolls[list])
                .w(px(120.))
                .h(px(180.))
        });
        let group = self
            .sortable
            .group("board", lists)
            .gap(px(20.))
            .on_drop(
                cx.listener(|view, event: &sortable::Move<usize, &'static str>, _, cx| {
                    let item = view.lists[event.from.list].remove(event.from.index);
                    assert_eq!(item, event.item);
                    view.lists[event.to.list].insert(event.to.index, item);
                    view.moves.push(event.clone());
                    cx.notify();
                }),
            )
            .on_drop_outside(cx.listener(
                |view, event: &sortable::OutsideDrop<&'static str>, _, _| {
                    view.outside.push(event.clone());
                },
            ));
        div()
            .id("clip")
            .w(px(500.))
            .h(px(if self.clipped { 90. } else { 250. }))
            .overflow_scroll()
            .track_scroll(&self.parent_scroll)
            .child(group)
    }
}

fn open(cx: &mut TestAppContext) -> (Entity<Board>, VisualTestContext) {
    let window = cx.add_window(|_, cx| Board {
        sortable: Sortable::new(motion::Painter::of(cx)),
        lists: vec![vec!["a", "b", "c"], vec!["d", "e"], vec![]],
        kinds: [""; 3],
        outside: Vec::new(),
        moves: Vec::new(),
        scrolls: (0..3).map(|_| ScrollHandle::new()).collect(),
        parent_scroll: ScrollHandle::new(),
        clicks: 0,
        axis: Axis::Vertical,
        clipped: false,
    });
    let view = window.root(cx).unwrap();
    let visual = VisualTestContext::from_window(window.into(), cx);
    visual.simulate_resize(size(px(500.), px(300.)));
    visual.run_until_parked();
    (view, visual)
}

fn down(x: f32, y: f32, cx: &mut VisualTestContext) {
    let at = point(px(x), px(y));
    cx.simulate_mouse_move(at, None, Modifiers::default());
    cx.simulate_mouse_down(at, MouseButton::Left, Modifiers::default());
}

fn travel(x: f32, y: f32, cx: &mut VisualTestContext) {
    cx.simulate_mouse_move(point(px(x), px(y)), MouseButton::Left, Modifiers::default());
    cx.run_until_parked();
}

fn up(x: f32, y: f32, cx: &mut VisualTestContext) {
    cx.simulate_mouse_up(point(px(x), px(y)), MouseButton::Left, Modifiers::default());
    cx.run_until_parked();
}

fn settle(cx: &mut VisualTestContext) {
    cx.executor().advance_clock(Duration::from_secs(1));
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
}

fn origin(id: &'static str, cx: &mut VisualTestContext) -> Point<Pixels> {
    cx.debug_bounds(id).unwrap().origin
}

#[gpui::test]
fn vertical_preview_commits_once_with_variable_heights(cx: &mut TestAppContext) {
    let (view, mut cx) = open(cx);
    down(10., 10., &mut cx);
    travel(10., 150., &mut cx);
    settle(&mut cx);
    assert_eq!(origin("a", &mut cx), point(px(0.), px(140.)));
    assert_eq!(origin("b", &mut cx).y, px(0.));
    assert_eq!(origin("c", &mut cx).y, px(84.));
    cx.update(|_, cx| {
        assert_eq!(view.read(cx).lists[0], ["a", "b", "c"]);
        assert!(view.read(cx).moves.is_empty());
        assert!(!cx.has_active_drag());
    });
    up(10., 150., &mut cx);
    cx.update(|_, cx| {
        let view = view.read(cx);
        assert_eq!(view.lists[0], ["b", "c", "a"]);
        assert_eq!(view.moves.len(), 1);
        assert_eq!(view.moves[0].to.index, 2);
        assert_eq!(view.clicks, 0);
    });
}

#[gpui::test]
fn entering_and_leaving_a_lane_moves_the_gap(cx: &mut TestAppContext) {
    let (view, mut cx) = open(cx);
    down(10., 10., &mut cx);
    travel(150., 10., &mut cx);
    settle(&mut cx);
    assert_eq!(origin("b", &mut cx).y, px(0.));
    assert_eq!(origin("d", &mut cx).y, px(44.));
    assert_eq!(origin("a", &mut cx), point(px(140.), px(0.)));
    travel(290., 30., &mut cx);
    settle(&mut cx);
    assert_eq!(origin("d", &mut cx).y, px(0.));
    up(290., 30., &mut cx);
    cx.update(|_, cx| {
        let view = view.read(cx);
        assert_eq!(view.lists, [vec!["b", "c"], vec!["d", "e"], vec!["a"]]);
        assert_eq!(view.moves.len(), 1);
        assert_eq!(view.moves[0].to, sortable::Position { list: 2, index: 0 });
    });
}

#[gpui::test]
fn dropping_after_the_last_item_uses_destination_indices(cx: &mut TestAppContext) {
    let (view, mut cx) = open(cx);
    down(10., 10., &mut cx);
    travel(150., 120., &mut cx);
    up(150., 120., &mut cx);
    cx.update(|_, cx| {
        assert_eq!(view.read(cx).lists[1], ["d", "e", "a"]);
        assert_eq!(view.read(cx).moves[0].to.index, 2);
    });
}

#[gpui::test]
fn outside_release_rolls_back_preview_without_mutating_data(cx: &mut TestAppContext) {
    let (view, mut cx) = open(cx);
    down(10., 10., &mut cx);
    travel(150., 15., &mut cx);
    up(450., 240., &mut cx);
    settle(&mut cx);
    cx.update(|_, cx| {
        assert!(view.read(cx).moves.is_empty());
        assert_eq!(view.read(cx).lists[0], ["a", "b", "c"]);
        assert_eq!(view.read(cx).outside.len(), 1);
        assert_eq!(view.read(cx).outside[0].item, "a");
        assert_eq!(view.read(cx).outside[0].position, point(px(450.), px(240.)));
    });
    assert_eq!(origin("a", &mut cx).y, px(0.));
    assert_eq!(origin("d", &mut cx).y, px(0.));
}

#[gpui::test]
fn clipped_areas_are_not_drop_targets(cx: &mut TestAppContext) {
    let (view, mut cx) = open(cx);
    view.update(&mut cx, |view, cx| {
        view.clipped = true;
        cx.notify();
    });
    cx.run_until_parked();
    down(10., 10., &mut cx);
    travel(150., 120., &mut cx);
    up(150., 120., &mut cx);
    cx.update(|_, cx| assert!(view.read(cx).moves.is_empty()));
}

#[gpui::test]
fn external_membership_changes_cancel_stale_indices(cx: &mut TestAppContext) {
    let (view, mut cx) = open(cx);
    down(10., 10., &mut cx);
    travel(150., 20., &mut cx);
    view.update(&mut cx, |view, cx| {
        view.lists[0].remove(0);
        cx.notify();
    });
    cx.run_until_parked();
    up(150., 20., &mut cx);
    cx.update(|_, cx| assert!(view.read(cx).moves.is_empty()));
}

#[gpui::test]
fn reduced_motion_opens_the_gap_immediately(cx: &mut TestAppContext) {
    let (_, mut cx) = open(cx);
    cx.update(|_, cx| cx.set_reduce_motion(true));
    down(10., 10., &mut cx);
    travel(150., 10., &mut cx);
    assert_eq!(origin("d", &mut cx).y, px(44.));
    assert_eq!(origin("b", &mut cx).y, px(0.));
}

#[gpui::test]
fn scrolling_during_a_drag_updates_aim_with_a_stationary_pointer(cx: &mut TestAppContext) {
    let (view, mut cx) = open(cx);
    view.update(&mut cx, |view, cx| {
        view.lists[1] = vec!["d", "e", "f", "g", "h", "i"];
        cx.notify();
    });
    cx.run_until_parked();
    down(10., 10., &mut cx);
    travel(150., 80., &mut cx);
    view.update(&mut cx, |view, cx| {
        view.scrolls[1].set_offset(point(px(0.), px(-60.)));
        cx.notify();
    });
    cx.run_until_parked();
    up(150., 80., &mut cx);
    cx.update(|_, cx| {
        assert_eq!(view.read(cx).moves.len(), 1);
        assert!(view.read(cx).moves[0].to.index >= 2);
    });
}

#[gpui::test]
fn escape_cancels_without_a_drop(cx: &mut TestAppContext) {
    let (view, mut cx) = open(cx);
    down(10., 10., &mut cx);
    travel(150., 20., &mut cx);
    cx.simulate_keystrokes("escape");
    up(150., 20., &mut cx);
    cx.update(|_, cx| assert!(view.read(cx).moves.is_empty()));
}

#[gpui::test]
fn horizontal_lists_share_the_same_drop_contract(cx: &mut TestAppContext) {
    let (view, mut cx) = open(cx);
    view.update(&mut cx, |view, cx| {
        view.axis = Axis::Horizontal;
        cx.notify();
    });
    cx.run_until_parked();
    down(10., 10., &mut cx);
    travel(150., 10., &mut cx);
    up(150., 10., &mut cx);
    cx.update(|_, cx| {
        assert_eq!(view.read(cx).lists[0], ["b", "c"]);
        assert_eq!(view.read(cx).lists[1], ["a", "d", "e"]);
    });
}

#[gpui::test]
fn stationary_edge_drag_scrolls_only_the_target_list_and_stops_on_release(cx: &mut TestAppContext) {
    use motion::AppExt as _;
    let (view, mut cx) = open(cx);
    cx.update(|_, cx| cx.set_pause_when_inactive(false));
    view.update(&mut cx, |view, cx| {
        view.lists[1] = vec!["d", "e", "f", "g", "h", "i", "j", "k"];
        cx.notify();
    });
    cx.run_until_parked();
    down(10., 10., &mut cx);
    travel(150., 175., &mut cx);
    for _ in 0..20 {
        cx.executor().advance_clock(Duration::from_millis(10));
        cx.run_until_parked();
    }
    let offset = cx.update(|_, cx| {
        let view = view.read(cx);
        assert_eq!(view.scrolls[0].offset(), Point::default());
        view.scrolls[1].offset().y
    });
    assert!(
        offset < px(-20.),
        "stationary pointer must drive scrolling: {offset:?}"
    );
    up(150., 175., &mut cx);
    let stopped = cx.update(|_, cx| view.read(cx).scrolls[1].offset());
    for _ in 0..20 {
        cx.executor().advance_clock(Duration::from_millis(10));
        cx.run_until_parked();
    }
    assert_eq!(
        cx.update(|_, cx| view.read(cx).scrolls[1].offset()),
        stopped
    );
}

#[gpui::test]
fn no_op_release_does_not_emit_a_move(cx: &mut TestAppContext) {
    let (view, mut cx) = open(cx);
    down(10., 10., &mut cx);
    travel(15., 15., &mut cx);
    up(15., 15., &mut cx);
    cx.update(|_, cx| {
        assert!(view.read(cx).moves.is_empty());
        assert_eq!(view.read(cx).clicks, 0);
    });
}

#[gpui::test]
fn a_drag_restores_the_previous_keyboard_focus(cx: &mut TestAppContext) {
    let (_, mut cx) = open(cx);
    let previous = cx.update(|window, cx| {
        let focus = cx.focus_handle();
        window.focus(&focus, cx);
        focus
    });
    down(10., 10., &mut cx);
    travel(150., 15., &mut cx);
    assert!(!cx.update(|window, _| previous.is_focused(window)));
    up(150., 15., &mut cx);
    assert!(cx.update(|window, _| previous.is_focused(window)));
}

#[gpui::test]
fn ancestor_scrolling_moves_slots_without_sliding_and_keeps_the_grab_point(
    cx: &mut TestAppContext,
) {
    let (view, mut cx) = open(cx);
    view.update(&mut cx, |view, cx| {
        view.clipped = true;
        cx.notify();
    });
    cx.run_until_parked();
    down(10., 10., &mut cx);
    travel(150., 50., &mut cx);
    settle(&mut cx);
    view.update(&mut cx, |view, cx| {
        view.parent_scroll.set_offset(point(px(0.), px(-20.)));
        cx.notify();
    });
    cx.run_until_parked();
    assert_eq!(origin("d", &mut cx).y, px(-20.));
    assert_eq!(origin("a", &mut cx).y, px(40.));
    up(150., 50., &mut cx);
    cx.update(|_, cx| assert_eq!(view.read(cx).lists[1], ["d", "a", "e"]));
}

#[gpui::test]
fn entries_project_headings_and_space_headings_reject_each_other(cx: &mut TestAppContext) {
    let (view, mut cx) = open(cx);
    view.update(&mut cx, |view, cx| {
        view.kinds = ["entries", "project-headings", "space-headings"];
        view.lists = vec![vec!["a", "b"], vec!["d", "e"], vec!["f", "g"]];
        cx.notify();
    });
    cx.run_until_parked();
    let first = ["a", "d", "f"];
    for (source, &item) in first.iter().enumerate() {
        for (target, &target_item) in first.iter().enumerate() {
            if source == target {
                continue;
            }
            let x = target as f32 * 140. + 10.;
            down(source as f32 * 140. + 10., 10., &mut cx);
            travel(x, 15., &mut cx);
            settle(&mut cx);
            assert_eq!(
                origin(target_item, &mut cx).y,
                px(0.),
                "incompatible list opened a gap"
            );
            up(x, 15., &mut cx);
            settle(&mut cx);
            cx.update(|_, cx| {
                let view = view.read(cx);
                assert!(view.moves.is_empty());
                let outside = view.outside.last().unwrap();
                assert_eq!(outside.item, item);
                assert_eq!(outside.position, point(px(x), px(15.)));
            });
        }
    }
    cx.update(|_, cx| {
        assert_eq!(view.read(cx).outside.len(), 6);
        assert_eq!(
            view.read(cx).lists,
            [vec!["a", "b"], vec!["d", "e"], vec!["f", "g"]]
        );
    });
    // Each kind can still reorder its own list.
    for source in 0..3 {
        let x = source as f32 * 140. + 10.;
        down(x, 10., &mut cx);
        travel(x, 110., &mut cx);
        up(x, 110., &mut cx);
        settle(&mut cx);
    }
    cx.update(|_, cx| {
        assert_eq!(view.read(cx).moves.len(), 3);
        assert_eq!(
            view.read(cx).lists,
            [vec!["b", "a"], vec!["e", "d"], vec!["g", "f"]]
        );
    });
}

#[gpui::test]
fn a_drag_passes_over_an_incompatible_list_into_a_matching_empty_list(cx: &mut TestAppContext) {
    let (view, mut cx) = open(cx);
    view.update(&mut cx, |view, cx| {
        view.kinds = ["entries", "headings", "entries"];
        cx.notify();
    });
    cx.run_until_parked();
    down(10., 10., &mut cx);
    travel(150., 15., &mut cx);
    settle(&mut cx);
    assert_eq!(origin("d", &mut cx).y, px(0.));
    assert_eq!(origin("a", &mut cx), point(px(140.), px(5.)));
    travel(290., 15., &mut cx);
    up(290., 15., &mut cx);
    cx.update(|_, cx| {
        assert_eq!(view.read(cx).lists[2], ["a"]);
        assert_eq!(view.read(cx).moves.len(), 1);
        assert!(view.read(cx).outside.is_empty());
    });
}

#[gpui::test]
fn entering_an_incompatible_list_closes_the_previous_gap(cx: &mut TestAppContext) {
    let (view, mut cx) = open(cx);
    view.update(&mut cx, |view, cx| {
        view.kinds = ["entries", "entries", "headings"];
        cx.notify();
    });
    cx.run_until_parked();
    down(10., 10., &mut cx);
    travel(150., 10., &mut cx);
    settle(&mut cx);
    assert_eq!(origin("d", &mut cx).y, px(44.));
    travel(290., 10., &mut cx);
    settle(&mut cx);
    assert_eq!(origin("d", &mut cx).y, px(0.));
    up(290., 10., &mut cx);
    cx.update(|_, cx| {
        assert!(view.read(cx).moves.is_empty());
        assert_eq!(view.read(cx).outside.len(), 1);
    });
}

#[gpui::test]
fn incompatible_lists_never_edge_scroll_and_matching_lists_still_do(cx: &mut TestAppContext) {
    use motion::AppExt as _;
    let (view, mut cx) = open(cx);
    cx.update(|_, cx| cx.set_pause_when_inactive(false));
    view.update(&mut cx, |view, cx| {
        view.kinds = ["entries", "headings", "entries"];
        view.lists[1] = vec!["d", "e", "f", "g", "h", "i", "j", "k"];
        view.lists[2] = vec!["l", "m", "n", "o", "p", "q", "r", "s"];
        cx.notify();
    });
    cx.run_until_parked();
    down(10., 10., &mut cx);
    for x in [150., 290.] {
        travel(x, 175., &mut cx);
        for _ in 0..20 {
            cx.executor().advance_clock(Duration::from_millis(10));
            cx.run_until_parked();
        }
        cx.update(|_, cx| assert_eq!(view.read(cx).scrolls[1].offset(), Point::default()));
    }
    cx.update(|_, cx| assert!(view.read(cx).scrolls[2].offset().y < px(-20.)));
    up(290., 175., &mut cx);
    cx.update(|_, cx| assert_eq!(view.read(cx).moves[0].to.list, 2));
}

#[gpui::test]
fn changing_a_lists_kind_cancels_the_pending_drop(cx: &mut TestAppContext) {
    let (view, mut cx) = open(cx);
    down(10., 10., &mut cx);
    travel(150., 15., &mut cx);
    view.update(&mut cx, |view, cx| {
        view.kinds[1] = "headings";
        cx.notify();
    });
    cx.run_until_parked();
    up(150., 15., &mut cx);
    cx.update(|_, cx| {
        assert!(view.read(cx).moves.is_empty());
        assert!(view.read(cx).outside.is_empty());
        assert_eq!(view.read(cx).lists[0], ["a", "b", "c"]);
    });
}
