use crate::{
    LoadState, WebViewEvent,
    page::{
        Report,
        platform::{Event, Message},
    },
    view::WebView,
};
use gpui::{
    self, Bounds, ClipboardItem, Context, EntityInputHandler, Pixels, UTF16Selection, Window,
};
use serde_json::json;
use std::ops::Range;
use ui::{context_menu::Place, menu::Item};

impl WebView {
    pub(super) fn linux_event(&mut self, message: Message, cx: &mut Context<Self>) {
        if message.epoch
            != self
                .page
                .platform
                .route
                .epoch
                .load(std::sync::atomic::Ordering::Acquire)
        {
            return;
        }
        let epoch = message.epoch;
        match message.event {
            Event::Evaluated(report) => self.report(*report, cx),
            Event::Console { message, _slot } => {
                cx.emit(WebViewEvent::Console(message));
            }
            Event::Ready(session) => {
                self.page.platform.ready(session);
                if self.page.placed.get().is_none() {
                    self.page
                        .platform
                        .command(json!({"cmd":"visible","value":0}));
                }
                cx.notify();
            }
            Event::Changed => {
                let state = self.page.platform.snapshot();
                if let Some(url) = state.url {
                    if self.location.as_deref() != Some(url.as_str()) {
                        ui::context_menu::ContextMenu::close(self, |this| &mut this.menu, cx);
                        self.page.platform.command(json!({"cmd":"dismiss-menu"}));
                    }
                    self.locate(url, cx);
                }
                self.report(Report::Title(state.title), cx);
                let loading = state.loading;
                if loading != self.loading {
                    self.loading = loading;
                    cx.emit(WebViewEvent::Load(if loading {
                        LoadState::Started
                    } else {
                        LoadState::Finished
                    }));
                }
                self.step(cx);
                self.error = state.error;
                cx.notify();
            }
            Event::Error(error) => {
                self.page.platform.failed();
                self.loading = false;
                self.error = Some(error.clone());
                cx.emit(WebViewEvent::Error(error));
                cx.notify();
            }
            Event::Clipboard(text) => cx.write_to_clipboard(ClipboardItem::new_string(text)),
            Event::Menu(menu) => {
                let view = cx.entity().downgrade();
                let Some(handle) = self.page.window() else {
                    return;
                };
                gpui::App::defer(cx, move |cx| {
                    let _ = handle.update(cx, |_, window, cx| {
                        if let Some(view) = view.upgrade() {
                            view.update(cx, |this, cx| {
                                if this
                                    .page
                                    .platform
                                    .route
                                    .epoch
                                    .load(std::sync::atomic::Ordering::Acquire)
                                    != epoch
                                {
                                    return;
                                }
                                if this.page.placed.get().is_none() {
                                    return;
                                }
                                if !this.page.input.enabled.get() {
                                    return;
                                }
                                let x = menu["x"].as_f64().unwrap_or(0.) as f32;
                                let y = menu["y"].as_f64().unwrap_or(0.) as f32;
                                let Some(position) = this
                                    .page
                                    .screen_position(gpui::point(gpui::px(x), gpui::px(y)))
                                else {
                                    return;
                                };
                                let Some(entries) = menu["items"].as_array() else {
                                    return;
                                };
                                let actions: Vec<_> = entries
                                    .iter()
                                    .map(|item| item["action"].as_str().unwrap_or("").to_owned())
                                    .collect();
                                let items = entries
                                    .iter()
                                    .map(|item| {
                                        Item::action(
                                            item["label"].as_str().unwrap_or("").to_owned(),
                                        )
                                        .disabled(item["enabled"].as_bool() != Some(true))
                                        .checked(item["selected"].as_bool() == Some(true))
                                    })
                                    .collect();
                                this.menu.open(
                                    (),
                                    Place::At(position),
                                    items,
                                    move |this, path, _, cx| {
                                        if let Some(action) =
                                            path.first().and_then(|index| actions.get(*index))
                                        {
                                            this.page.platform.share_clipboard(cx);
                                            this.page.platform.command(json!({"cmd":action}));
                                        }
                                    },
                                    window,
                                    cx,
                                );
                            });
                        }
                    });
                });
            }
        }
    }
}

impl EntityInputHandler for WebView {
    fn text_for_range(
        &mut self,
        range: Range<usize>,
        actual: &mut Option<Range<usize>>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<String> {
        let input = self.page.platform.route.input.lock().unwrap();
        let text = input["text"].as_str().unwrap_or("");
        let units: Vec<_> = text.encode_utf16().collect();
        let range = range.start.min(units.len())..range.end.min(units.len());
        *actual = Some(range.clone());
        Some(String::from_utf16_lossy(units.get(range)?))
    }
    fn selected_text_range(
        &mut self,
        _: bool,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        let input = self.page.platform.route.input.lock().unwrap();
        let text = input["text"].as_str().unwrap_or("");
        let offset = |key: &str| {
            let byte = input[key].as_u64().unwrap_or(0) as usize;
            text.get(..byte).unwrap_or(text).encode_utf16().count()
        };
        let cursor = offset("cursor");
        let selection = offset("selection");
        Some(UTF16Selection {
            range: cursor.min(selection)..cursor.max(selection),
            reversed: cursor < selection,
        })
    }
    fn marked_text_range(&self, _: &mut Window, _: &mut Context<Self>) -> Option<Range<usize>> {
        let marked = self.page.platform.preedit.borrow();
        (!marked.is_empty()).then(|| 0..marked.encode_utf16().count())
    }
    fn unmark_text(&mut self, _: &mut Window, _: &mut Context<Self>) {
        self.page.platform.command(json!({"cmd":"unmark"}));
        self.page.platform.preedit.borrow_mut().clear();
    }
    fn replace_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        _: &mut Window,
        _: &mut Context<Self>,
    ) {
        if self.page.input.enabled.get() {
            self.page.platform.replace_input(range, text, false);
        }
    }
    fn replace_and_mark_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        _: Option<Range<usize>>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) {
        if self.page.input.enabled.get() {
            self.page.platform.replace_input(range, text, true);
        }
    }
    fn bounds_for_range(
        &mut self,
        _: Range<usize>,
        _: Bounds<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        self.page.caret_bounds()
    }
    fn character_index_for_point(
        &mut self,
        _: gpui::Point<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<usize> {
        None
    }
    fn accepts_text_input(&self, _: &mut Window, _: &mut Context<Self>) -> bool {
        self.page.input.enabled.get()
            && self.page.platform.route.input.lock().unwrap()["focused"].as_bool() == Some(true)
    }
}
