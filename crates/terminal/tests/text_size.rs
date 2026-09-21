//! Font sizing reaches the measured grid, including after resetting the size.
use gpui::{Context, Render, TestAppContext, VisualTestContext, Window, px, size};
use std::{cell::Cell, rc::Rc};
use terminal::view::{GridGeometry, GridSnapshot, TerminalElement};

struct Grid {
    size: f32,
    geometry: Rc<Cell<Option<GridGeometry>>>,
}

impl Render for Grid {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl gpui::IntoElement {
        let geometry = self.geometry.clone();
        TerminalElement::new(
            move |grid, _| {
                geometry.set(Some(grid));
                Some(GridSnapshot {
                    images: Vec::new(),
                    lines: Vec::new(),
                    cursor: None,
                })
            },
            true,
        )
        .with_text_size(self.size)
    }
}

#[gpui::test]
fn zoom_remeasures_cells_lines_and_available_rows(cx: &mut TestAppContext) {
    cx.update(|cx| theme::Theme::install(theme::Appearance::Dark, cx));
    let geometry = Rc::new(Cell::new(None));
    let window = cx.add_window(|_, _| Grid {
        size: 13.,
        geometry: geometry.clone(),
    });
    let grid = window.root(cx).unwrap();
    let mut cx = VisualTestContext::from_window(window.into(), cx);
    cx.simulate_resize(size(px(600.), px(300.)));
    cx.run_until_parked();
    let before = geometry.get().expect("grid measured");
    cx.update(|_, cx| {
        grid.update(cx, |grid, cx| {
            grid.size = 26.;
            cx.notify();
        })
    });
    cx.run_until_parked();
    let after = geometry.get().unwrap();
    assert!(after.cell_w > before.cell_w);
    assert_eq!(after.line_h, before.line_h * 2.);
    assert!(after.rows < before.rows);
    assert!(after.cols < before.cols);
    cx.update(|_, cx| {
        grid.update(cx, |grid, cx| {
            grid.size = 13.;
            cx.notify();
        })
    });
    cx.run_until_parked();
    assert_eq!(geometry.get().unwrap(), before);
}
