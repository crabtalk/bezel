use std::time::Duration;

use gpui::{
    Axis, Context, Entity, Modifiers, MouseButton, Render, ScrollHandle, TestAppContext,
    VisualTestContext, Window, div, point, prelude::*, px, size,
};
use ui::{
    docking::{self, Dock, Zone},
    drag::{self, Domain, Feedback},
    tabs,
};

struct Board {
    domain: Domain<usize, &'static str>,
    lanes: Vec<Vec<&'static str>>,
    scrolls: Vec<ScrollHandle>,
    only: Option<(usize, Vec<&'static str>)>,
    indicator: Option<usize>,
    drops: Vec<drag::Drop<usize, &'static str>>,
    outside: Vec<&'static str>,
}

fn extent(id: &str) -> f32 {
    match id {
        "b" => 80.,
        "d" => 60.,
        _ => 40.,
    }
}

impl Render for Board {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let lanes = self.lanes.iter().enumerate().map(|(lane, items)| {
            let content = div()
                .id(("content", lane))
                .size_full()
                .flex()
                .flex_col()
                .gap(px(4.))
                .overflow_y_scroll()
                .track_scroll(&self.scrolls[lane])
                .children(items.iter().map(|&id| {
                    self.domain.handle(
                        id,
                        div()
                            .id(id)
                            .debug_selector(move || id.into())
                            .flex_none()
                            .w_full()
                            .h(px(extent(id))),
                    )
                }));
            let mut region = self
                .domain
                .region(("lane", lane), lane, Axis::Vertical, content)
                .track_scroll(&self.scrolls[lane])
                .w(px(120.))
                .h(px(180.))
                .on_drop(
                    cx.listener(|view, event: &drag::Drop<usize, &'static str>, _, cx| {
                        view.drops.push(event.clone());
                        cx.notify();
                    }),
                )
                .on_drop_outside(
                    cx.listener(|view, event: &drag::Outside<&'static str>, _, _| {
                        view.outside.push(event.item);
                    }),
                );
            if let Some((only, accepted)) = self.only.clone()
                && only == lane
            {
                region = region.accepts(move |item| accepted.contains(item));
            }
            if self.indicator == Some(lane) {
                region = region.feedback(Feedback::Indicator);
            }
            region
        });
        div().flex().gap(px(20.)).children(lanes)
    }
}

fn open(
    lanes: Vec<Vec<&'static str>>,
    cx: &mut TestAppContext,
) -> (Entity<Board>, VisualTestContext) {
    cx.update(|cx| theme::Theme::install(theme::Appearance::Dark, cx));
    let count = lanes.len();
    let window = cx.add_window(|_, cx| Board {
        domain: Domain::new(motion::Painter::of(cx)),
        lanes,
        scrolls: (0..count).map(|_| ScrollHandle::new()).collect(),
        only: None,
        indicator: None,
        drops: Vec::new(),
        outside: Vec::new(),
    });
    let view = window.root(cx).unwrap();
    let visual = VisualTestContext::from_window(window.into(), cx);
    visual.simulate_resize(size(px(500.), px(300.)));
    visual.run_until_parked();
    (view, visual)
}

fn board(cx: &mut TestAppContext) -> (Entity<Board>, VisualTestContext) {
    open(vec![vec!["a", "b", "c"], vec!["d", "e"], vec![]], cx)
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

#[gpui::test]
fn a_drop_names_its_neighbours_in_the_landing_region(cx: &mut TestAppContext) {
    let (view, mut cx) = board(cx);
    down(10., 10., &mut cx);
    travel(150., 72., &mut cx);
    up(150., 72., &mut cx);
    cx.update(|_, cx| {
        assert_eq!(
            view.read(cx).drops,
            [drag::Drop {
                item: "a",
                from: 0,
                region: 1,
                after: Some("d"),
                before: Some("e"),
            }]
        );
    });
}

#[gpui::test]
fn an_empty_region_takes_an_item_with_no_neighbours(cx: &mut TestAppContext) {
    let (view, mut cx) = board(cx);
    down(10., 10., &mut cx);
    travel(300., 20., &mut cx);
    up(300., 20., &mut cx);
    cx.update(|_, cx| {
        let drops = &view.read(cx).drops;
        assert_eq!(drops.len(), 1);
        assert_eq!(
            (drops[0].region, &drops[0].after, &drops[0].before),
            (2, &None, &None)
        );
    });
}

#[gpui::test]
fn the_landing_region_opens_a_gap_and_the_source_closes_one(cx: &mut TestAppContext) {
    let (_, mut cx) = board(cx);
    let e = cx.debug_bounds("e").unwrap().top();
    let b = cx.debug_bounds("b").unwrap().top();
    down(10., 10., &mut cx);
    travel(150., 72., &mut cx);
    settle(&mut cx);
    assert_eq!(cx.debug_bounds("e").unwrap().top(), e + px(44.));
    assert_eq!(cx.debug_bounds("b").unwrap().top(), b - px(44.));
    up(150., 72., &mut cx);
}

#[gpui::test]
fn an_indicator_region_leaves_its_items_in_place(cx: &mut TestAppContext) {
    let (view, mut cx) = board(cx);
    view.update(&mut cx, |view, _| view.indicator = Some(1));
    cx.run_until_parked();
    let e = cx.debug_bounds("e").unwrap().top();
    down(10., 10., &mut cx);
    travel(150., 72., &mut cx);
    settle(&mut cx);
    assert_eq!(cx.debug_bounds("e").unwrap().top(), e);
    up(150., 72., &mut cx);
    cx.update(|_, cx| assert_eq!(view.read(cx).drops[0].before, Some("e")));
}

#[gpui::test]
fn a_region_that_refuses_the_item_is_not_a_landing(cx: &mut TestAppContext) {
    let (view, mut cx) = board(cx);
    view.update(&mut cx, |view, _| view.only = Some((1, vec!["d", "e"])));
    cx.run_until_parked();
    down(10., 10., &mut cx);
    travel(150., 72., &mut cx);
    up(150., 72., &mut cx);
    cx.update(|_, cx| {
        assert!(view.read(cx).drops.is_empty());
        assert_eq!(view.read(cx).outside, ["a"]);
    });
}

#[gpui::test]
fn a_release_over_no_region_is_outside(cx: &mut TestAppContext) {
    let (view, mut cx) = board(cx);
    down(10., 10., &mut cx);
    travel(450., 250., &mut cx);
    up(450., 250., &mut cx);
    cx.update(|_, cx| {
        assert!(view.read(cx).drops.is_empty());
        assert_eq!(view.read(cx).outside, ["a"]);
    });
}

#[gpui::test]
fn a_release_where_the_item_started_commits_nothing(cx: &mut TestAppContext) {
    let (view, mut cx) = board(cx);
    down(10., 10., &mut cx);
    travel(10., 16., &mut cx);
    up(10., 16., &mut cx);
    cx.update(|_, cx| {
        assert!(view.read(cx).drops.is_empty());
        assert!(view.read(cx).outside.is_empty());
    });
}

#[gpui::test]
fn escape_ends_the_drag_without_a_drop(cx: &mut TestAppContext) {
    let (view, mut cx) = board(cx);
    down(10., 10., &mut cx);
    travel(150., 72., &mut cx);
    cx.simulate_keystrokes("escape");
    cx.run_until_parked();
    cx.update(|_, cx| assert!(!cx.has_active_drag()));
    up(150., 72., &mut cx);
    cx.update(|_, cx| {
        assert!(view.read(cx).drops.is_empty());
        assert!(view.read(cx).outside.is_empty());
    });
}

#[gpui::test]
fn holding_near_an_edge_scrolls_the_landing_region(cx: &mut TestAppContext) {
    let (view, mut cx) = open(vec![vec!["a", "b", "c", "f", "g"], vec!["d", "e"]], cx);
    down(150., 10., &mut cx);
    travel(60., 176., &mut cx);
    for _ in 0..6 {
        cx.executor().advance_clock(Duration::from_millis(50));
        cx.update(|window, _| window.refresh());
        cx.run_until_parked();
    }
    let scrolled = cx.update(|_, cx| view.read(cx).scrolls[0].offset().y);
    assert!(scrolled < px(0.));
    cx.update(|_, cx| assert_eq!(view.read(cx).scrolls[1].offset().y, px(0.)));
    up(60., 176., &mut cx);
    settle(&mut cx);
    cx.update(|_, cx| assert_eq!(view.read(cx).scrolls[0].offset().y, scrolled));
}

struct Workspace {
    dock: Dock<usize, &'static str>,
    strip: tabs::Strip<&'static str>,
    reorder: tabs::Reorder<&'static str>,
    docked: Vec<docking::Drop<usize, &'static str>>,
}

impl Render for Workspace {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = theme::Theme::of(cx);
        let tabs = self.strip.tabs().iter().map(|&id| {
            (
                id,
                tabs::tab(theme, id, tabs::Label::new(id), tabs::State::Resting).w(px(80.)),
            )
        });
        let pane = self.dock.pane(
            0,
            px(26.),
            div()
                .size_full()
                .flex()
                .flex_col()
                .child(self.reorder.bar("strip", &self.strip, tabs)),
        );
        let owner = cx.entity().downgrade();
        self.dock
            .surface("workspace", div().w(px(400.)).h(px(300.)).child(pane))
            .on_drop(move |event, _, cx| {
                owner
                    .update(cx, |view, _| {
                        view.docked.push(event.clone());
                        Some(event.pane)
                    })
                    .ok()
                    .flatten()
            })
    }
}

#[gpui::test]
fn a_tab_pulled_off_its_strip_lands_on_a_docking_surface(cx: &mut TestAppContext) {
    cx.update(|cx| theme::Theme::install(theme::Appearance::Dark, cx));
    let window = cx.add_window(|_, cx| Workspace {
        dock: Dock::new(motion::Painter::of(cx), |_, _, _| div().into_any_element()),
        strip: ["x", "y"].into_iter().collect(),
        reorder: tabs::Reorder::new(motion::Painter::of(cx)),
        docked: Vec::new(),
    });
    let view = window.root(cx).unwrap();
    let mut cx = VisualTestContext::from_window(window.into(), cx);
    cx.simulate_resize(size(px(500.), px(400.)));
    cx.run_until_parked();
    down(10., 10., &mut cx);
    travel(20., 140., &mut cx);
    travel(20., 150., &mut cx);
    assert!(cx.debug_bounds("dock-preview").is_some());
    up(20., 150., &mut cx);
    cx.update(|_, cx| {
        assert_eq!(
            view.read(cx).docked,
            [docking::Drop {
                item: "x",
                pane: 0,
                zone: Zone::Left,
            }]
        );
    });
}

/// A heading, then two groups whose heading carries the rows under it, in a
/// virtualized list.
struct Outline {
    domain: Domain<(), &'static str>,
    rows: Vec<&'static str>,
    drops: Vec<drag::Drop<(), &'static str>>,
}

fn group_of(rows: &[&'static str], row: &str) -> Option<&'static str> {
    let at = rows.iter().position(|at| *at == row)?;
    rows[..=at]
        .iter()
        .rev()
        .find(|at| at.starts_with('G'))
        .copied()
}

impl Render for Outline {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let rows = self.rows.clone();
        let list = gpui::uniform_list(
            "outline",
            rows.len(),
            cx.processor(move |this, range: std::ops::Range<usize>, _, _| {
                range
                    .map(|ix| {
                        let row = this.rows[ix];
                        let el = div().id(row).debug_selector(move || row.into()).h(px(30.));
                        match row {
                            "H" => this.domain.fixed(row, el).into_any_element(),
                            _ => this.domain.handle(row, el).into_any_element(),
                        }
                    })
                    .collect::<Vec<_>>()
            }),
        )
        .size_full();
        let (carried, landing) = (rows.clone(), rows);
        self.domain
            .region("outline-region", (), Axis::Vertical, list)
            .w(px(120.))
            .h(px(300.))
            .carries(move |item| match item.starts_with('G') {
                true => carried
                    .iter()
                    .skip_while(|at| *at != item)
                    .skip(1)
                    .take_while(|at| !at.starts_with('G'))
                    .copied()
                    .collect(),
                false => Vec::new(),
            })
            .lands(move |item, after, before| match item.starts_with('G') {
                true => before.is_none_or(|before| before.starts_with('G')),
                false => [after, before].into_iter().flatten().any(|at| {
                    !at.starts_with('G') && group_of(&landing, at) == group_of(&landing, item)
                }),
            })
            .on_drop(
                cx.listener(|view, event: &drag::Drop<(), &'static str>, _, _| {
                    view.drops.push(event.clone());
                }),
            )
    }
}

fn outline(cx: &mut TestAppContext) -> (Entity<Outline>, VisualTestContext) {
    cx.update(|cx| theme::Theme::install(theme::Appearance::Dark, cx));
    let window = cx.add_window(|_, cx| Outline {
        domain: Domain::new(motion::Painter::of(cx)),
        rows: vec!["H", "G1", "a", "b", "G2", "c", "d"],
        drops: Vec::new(),
    });
    let view = window.root(cx).unwrap();
    let cx = VisualTestContext::from_window(window.into(), cx);
    cx.simulate_resize(size(px(300.), px(400.)));
    cx.run_until_parked();
    (view, cx)
}

#[gpui::test]
fn an_entry_in_a_virtualized_list_slides_its_neighbour_and_lands(cx: &mut TestAppContext) {
    let (view, mut cx) = outline(cx);
    let b = cx.debug_bounds("b").unwrap().top();
    down(10., 70., &mut cx);
    travel(10., 100., &mut cx);
    travel(10., 102., &mut cx);
    settle(&mut cx);
    assert_eq!(cx.debug_bounds("b").unwrap().top(), b - px(30.));
    up(10., 102., &mut cx);
    cx.update(|_, cx| {
        let drops = &view.read(cx).drops;
        assert_eq!((drops[0].after, drops[0].before), (Some("b"), Some("G2")));
    });
}

#[gpui::test]
fn a_heading_carries_its_rows_and_an_entry_stays_in_its_group(cx: &mut TestAppContext) {
    let (view, mut cx) = outline(cx);
    let g2 = cx.debug_bounds("G2").unwrap().top();
    down(10., 40., &mut cx);
    travel(10., 190., &mut cx);
    travel(10., 192., &mut cx);
    settle(&mut cx);
    assert_eq!(cx.debug_bounds("G2").unwrap().top(), g2 - px(90.));
    up(10., 192., &mut cx);
    cx.update(|_, cx| {
        let drops = &view.read(cx).drops;
        assert_eq!((drops[0].after, drops[0].before), (Some("d"), None));
    });
    settle(&mut cx);
    down(10., 70., &mut cx);
    travel(10., 190., &mut cx);
    travel(10., 192., &mut cx);
    up(10., 192., &mut cx);
    cx.update(|_, cx| {
        let drops = &view.read(cx).drops;
        assert_eq!(drops.len(), 2);
        assert_eq!(drops[1].before, Some("G2"));
    });
}
