#![cfg(target_os = "linux")]

use browser::{DataStore, Presentation, WebView, WebViewEvent};
use gpui::{TestAppContext, VisualTestContext, native_view::Viewport, px, size};
use std::{cell::RefCell, rc::Rc};
use theme::{Appearance, Theme};

#[gpui::test]
fn unsupported_browser_reports_errors_and_stays_disabled(cx: &mut TestAppContext) {
    cx.update(|cx| Theme::install(Appearance::Dark, cx));
    let window = cx.add_window(|window, cx| WebView::new("https://example.com", window, cx));
    let view = window.root(cx).unwrap();
    let errors = Rc::new(RefCell::new(Vec::new()));
    cx.update(|cx| {
        let errors = errors.clone();
        cx.subscribe(&view, move |_, event: &WebViewEvent, _| {
            if let WebViewEvent::Error(error) = event {
                errors.borrow_mut().push(error.clone());
            }
        })
        .detach();
    });
    let mut cx = VisualTestContext::from_window(window.into(), cx);
    cx.run_until_parked();
    for step in 0..4 {
        if step > 0 {
            cx.update(|_, cx| {
                view.update(cx, |view, _| match step {
                    1 => view.load("https://example.org"),
                    2 => view.reload(),
                    _ => view.reload_bypassing_cache(),
                });
            });
            cx.run_until_parked();
        }
        cx.update(|window, cx| {
            view.update(cx, |view, cx| {
                assert_eq!(
                    view.error.as_deref(),
                    Some("Embedded browsing is not supported on this platform")
                );
                assert!(!view.can_evaluate());
                assert!(!view.is_loading());
                assert!(!view.can_go_back());
                assert!(!view.can_go_forward());
                assert!(
                    view.set_presentation(
                        Some(Presentation {
                            viewport: Viewport::try_from(size(px(320.), px(200.))).unwrap(),
                            interactive: true,
                        }),
                        window,
                        cx,
                    )
                    .is_err()
                );
            });
        });
    }
    assert_eq!(errors.borrow().len(), 4);
    assert!(!DataStore::SUPPORTS_MANAGEMENT);
    assert!(!WebView::HAS_INSPECTOR);
}
