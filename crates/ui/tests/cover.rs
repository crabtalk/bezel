//! Which covers a native view under an overlay sees: those gpui prepaints after
//! the view's own mark, overlapping it, in the frame being drawn.

use std::{cell::Cell, rc::Rc};

use gpui::{TestAppContext, VisualTestContext, canvas, div, prelude::*, px, size};
use ui::cover;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Place {
    None,
    /// Overlapping the probe, before it in the tree.
    Before,
    /// Overlapping the probe, after it in the tree.
    After,
    /// After it in the tree, beside it.
    Beside,
    /// Overlapping the probe, in a deferred layer.
    Deferred,
}

struct Page {
    place: Place,
    seen: Rc<Cell<Option<bool>>>,
}

impl gpui::Render for Page {
    fn render(&mut self, _: &mut gpui::Window, _: &mut gpui::Context<Self>) -> impl IntoElement {
        let seen = self.seen.clone();
        let probe = canvas(
            |_, _, _| cover::mark(),
            move |bounds, mark, window, cx| {
                seen.set(Some(cover::covered(mark, bounds, window, cx)))
            },
        )
        .absolute()
        .top_0()
        .left_0()
        .size(px(100.0));
        let over = || {
            div()
                .absolute()
                .top(px(50.0))
                .left(px(50.0))
                .size(px(100.0))
        };
        let beside = div().absolute().top_0().left(px(200.0)).size(px(100.0));
        let mut page = div().size_full().relative();
        if self.place == Place::Before {
            page = page.child(over().child(cover::cover()));
        }
        page = page.child(probe);
        match self.place {
            Place::After => page.child(over().child(cover::cover())),
            Place::Beside => page.child(beside.child(cover::cover())),
            Place::Deferred => page.child(gpui::deferred(over().child(cover::cover()))),
            Place::None | Place::Before => page,
        }
    }
}

fn open(
    place: Place,
    cx: &mut TestAppContext,
) -> (
    gpui::Entity<Page>,
    Rc<Cell<Option<bool>>>,
    VisualTestContext,
) {
    let seen = Rc::new(Cell::new(None));
    let window = cx.add_window({
        let seen = seen.clone();
        move |_, _| Page { place, seen }
    });
    let view = window.root(cx).unwrap();
    let visual = VisualTestContext::from_window(window.into(), cx);
    visual.simulate_resize(size(px(400.0), px(300.0)));
    visual.run_until_parked();
    (view, seen, visual)
}

#[gpui::test]
fn a_cover_painted_over_the_view_covers_it(cx: &mut TestAppContext) {
    for place in [Place::After, Place::Deferred] {
        let (_, seen, _) = open(place, cx);
        assert_eq!(seen.get(), Some(true));
    }
}

#[gpui::test]
fn a_cover_under_or_beside_the_view_does_not(cx: &mut TestAppContext) {
    for place in [Place::None, Place::Before, Place::Beside] {
        let (_, seen, _) = open(place, cx);
        assert_eq!(seen.get(), Some(false));
    }
}

#[gpui::test]
fn a_cover_gone_from_the_frame_no_longer_covers(cx: &mut TestAppContext) {
    let (view, seen, mut visual) = open(Place::After, cx);
    assert_eq!(seen.get(), Some(true));
    view.update(&mut visual, |page, cx| {
        page.place = Place::None;
        cx.notify();
    });
    visual.run_until_parked();
    assert_eq!(seen.get(), Some(false));
}
