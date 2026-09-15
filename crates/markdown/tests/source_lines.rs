use gpui::{
    Context, Render, TestAppContext, VisualTestContext, Window, div, point, prelude::*, px, size,
};
use markdown::{BlockLayouts, Cursor, Editing, Part, render_source};

struct Page {
    source: String,
    layouts: BlockLayouts,
}

impl Render for Page {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div().w(px(320.0)).child(render_source(
            &self.source,
            Editing {
                layouts: Some(&self.layouts),
                ..Default::default()
            },
            cx,
        ))
    }
}

fn open(source: &str, cx: &mut TestAppContext) -> (gpui::Entity<Page>, VisualTestContext) {
    cx.update(|cx| theme::Theme::install(theme::Appearance::Dark, cx));
    let window = cx.add_window(|_, _| Page {
        source: source.into(),
        layouts: BlockLayouts::default(),
    });
    let page = window.root(cx).unwrap();
    let visual = VisualTestContext::from_window(window.into(), cx);
    visual.simulate_resize(size(px(320.0), px(600.0)));
    visual.run_until_parked();
    (page, visual)
}

#[gpui::test]
fn wrapped_and_empty_lines_keep_their_caret_and_hit_positions(cx: &mut TestAppContext) {
    let line = "word ".repeat(30);
    let source = format!("{line}\n\nlast\n");
    let (page, mut cx) = open(&source, cx);
    cx.update(|_, cx| {
        let layouts = &page.read(cx).layouts;
        let at = |offset| {
            layouts
                .position(Cursor::new(0, Part::Code, offset))
                .unwrap()
        };
        let (start, _) = at(0);
        let (end, height) = at(line.len());
        let (blank, _) = at(line.len() + 1);
        let (last, _) = at(line.len() + 2);
        let (trailing, _) = at(source.len());
        assert!(start.x > px(20.0), "text leaves room for the gutter");
        assert!(end.y > start.y, "long source lines still wrap");
        assert!(blank.y > end.y);
        assert_eq!(last.y - blank.y, height);
        assert_eq!(trailing.y - last.y, height);
        assert_eq!(last.x, start.x);
        for offset in [0, line.len() + 1, line.len() + 2, source.len()] {
            let (position, height) = at(offset);
            assert_eq!(
                layouts.hit(point(position.x, position.y + height / 2.0)),
                Some(Cursor::new(0, Part::Code, offset)),
            );
        }
    });
}

#[gpui::test]
fn gutter_grows_when_line_numbers_gain_a_digit(cx: &mut TestAppContext) {
    let (page, mut cx) = open(&vec!["x"; 9].join("\n"), cx);
    let before = cx.update(|_, cx| {
        page.read(cx)
            .layouts
            .position(Cursor::new(0, Part::Code, 0))
            .unwrap()
            .0
            .x
    });
    cx.update(|_, cx| {
        page.update(cx, |page, cx| {
            page.source.push_str("\nx");
            cx.notify();
        })
    });
    cx.run_until_parked();
    cx.update(|_, cx| {
        let layouts = &page.read(cx).layouts;
        let first = layouts.position(Cursor::new(0, Part::Code, 0)).unwrap().0;
        let last = layouts.position(Cursor::new(0, Part::Code, 18)).unwrap().0;
        assert!(first.x > before);
        assert_eq!(last.x, first.x);
    });
}

#[gpui::test]
fn host_styles_resize_and_hide_the_gutter(cx: &mut TestAppContext) {
    let (page, mut cx) = open("first\nlast", cx);
    let x = |page: &gpui::Entity<Page>, cx: &mut gpui::App| {
        page.read(cx)
            .layouts
            .position(Cursor::new(0, Part::Code, 0))
            .unwrap()
            .0
            .x
    };
    let before = cx.update(|_, cx| x(&page, cx));
    cx.update(|_, cx| {
        markdown::set_source_style(cx, |_| markdown::SourceStyle {
            gutter_min_digits: 4,
            gutter_gap: 2.0,
            ..Default::default()
        })
    });
    cx.run_until_parked();
    assert!(cx.update(|_, cx| x(&page, cx)) > before);
    cx.update(|_, cx| {
        markdown::set_source_style(cx, |_| markdown::SourceStyle {
            line_numbers: false,
            ..Default::default()
        })
    });
    cx.run_until_parked();
    assert!(cx.update(|_, cx| x(&page, cx)) < before);
}

#[gpui::test]
fn host_colors_follow_theme_changes(cx: &mut TestAppContext) {
    cx.update(|cx| {
        markdown::set_source_style(cx, |theme| markdown::SourceStyle {
            gutter_color: Some(theme.text_muted),
            ..Default::default()
        });
        for appearance in [theme::Appearance::Dark, theme::Appearance::Light] {
            theme::Theme::install(appearance, cx);
            assert_eq!(
                markdown::SourceStyle::of(cx).gutter_color,
                Some(theme::Theme::of(cx).text_muted),
            );
        }
    });
}
