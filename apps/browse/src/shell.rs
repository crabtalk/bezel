//! The window's one view: toolbar, tab bar, sidebar and the front tab's page.
//!
//! Only the front tab's `WebView` is in the tree; every other tab's page is
//! parked and stays loaded.

use browser::{WebView, WebViewEvent};
use gpui::{
    AnyElement, App, Context, Entity, FocusHandle, Focusable, KeyBinding, SharedString,
    Subscription, Window, actions, div, prelude::*, px, relative,
};
use theme::{TextStyle, Theme, Typeset};
use ui::{
    icons,
    input::{SelectAll, TextField},
    tabs::Strip,
    titlebar,
    widgets::{ButtonStyle, Buttons},
};

actions!(
    browse,
    [
        NewTab,
        CloseTab,
        NextTab,
        PreviousTab,
        FocusAddress,
        Reload,
        Back,
        Forward,
        ToggleSidebar,
        Go,
        Cancel,
    ]
);

const CONTEXT: &str = "Browser";
/// Claimed by the address field on top of `TextField`.
const ADDRESS_CONTEXT: &str = "Address";

/// The traffic lights' top-left corner, centred on the toolbar.
pub const TRAFFIC_LIGHT_X: f32 = 20.0;
pub const TRAFFIC_LIGHT_Y: f32 = (Theme::HEADER_HEIGHT - TRAFFIC_LIGHT_SIZE) / 2.0;
const TRAFFIC_LIGHT_SIZE: f32 = 14.0;
/// Centre-to-centre spacing of the three buttons on macOS 26.
const TRAFFIC_LIGHT_PITCH: f32 = 23.0;
/// What the toolbar leaves at its leading edge for the traffic lights.
const LIGHTS_ROOM: f32 = if cfg!(target_os = "macos") {
    TRAFFIC_LIGHT_X * 2.0 + TRAFFIC_LIGHT_PITCH * 2.0 + TRAFFIC_LIGHT_SIZE
} else {
    8.0
};

const TAB_BAR_HEIGHT: f32 = 30.0;
const ADDRESS_HEIGHT: f32 = 30.0;
const ADDRESS_MAX_WIDTH: f32 = 640.0;
const RELOAD_SIZE: f32 = 22.0;
const SIDEBAR_WIDTH: f32 = 220.0;
const TILE: f32 = 64.0;

const SEARCH: &str = "https://duckduckgo.com/?q=";

const FAVORITES: [(&str, &str); 8] = [
    ("Apple", "https://www.apple.com"),
    ("Wikipedia", "https://www.wikipedia.org"),
    ("GitHub", "https://github.com"),
    ("Hacker News", "https://news.ycombinator.com"),
    ("Rust", "https://www.rust-lang.org"),
    ("docs.rs", "https://docs.rs"),
    ("MDN", "https://developer.mozilla.org"),
    ("Example", "https://example.com"),
];

pub fn init(cx: &mut App) {
    let app = Some(CONTEXT);
    let address = Some(ADDRESS_CONTEXT);
    cx.bind_keys([
        KeyBinding::new("secondary-t", NewTab, app),
        KeyBinding::new("secondary-w", CloseTab, app),
        KeyBinding::new("secondary-l", FocusAddress, app),
        KeyBinding::new("secondary-r", Reload, app),
        KeyBinding::new("secondary-[", Back, app),
        KeyBinding::new("secondary-]", Forward, app),
        KeyBinding::new("secondary-shift-l", ToggleSidebar, app),
        KeyBinding::new("ctrl-tab", NextTab, app),
        KeyBinding::new("ctrl-shift-tab", PreviousTab, app),
        KeyBinding::new("secondary-shift-]", NextTab, app),
        KeyBinding::new("secondary-shift-[", PreviousTab, app),
        KeyBinding::new("enter", Go, address),
        KeyBinding::new("escape", Cancel, address),
    ]);
}

struct Tab {
    id: u64,
    /// `None` while the tab shows the start page.
    page: Option<(Entity<WebView>, Subscription)>,
}

impl Tab {
    fn page(&self) -> Option<&Entity<WebView>> {
        self.page.as_ref().map(|(page, _)| page)
    }
}

pub struct Shell {
    focus: FocusHandle,
    tabs: Vec<Tab>,
    strip: Strip<u64>,
    next: u64,
    address: Entity<TextField>,
    sidebar: bool,
    drag: titlebar::DragState,
    _address: Subscription,
}

impl Shell {
    pub fn new(url: Option<String>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let address = cx.new(|cx| {
            TextField::new(cx)
                .with_frame(false)
                .with_key_context(ADDRESS_CONTEXT)
                .with_placeholder("Search or enter website name")
        });
        // The toolbar shows the field only while it is focused.
        let blur = cx.on_blur(&address.focus_handle(cx), window, |_, _, cx| cx.notify());
        let mut shell = Self {
            focus: cx.focus_handle(),
            tabs: Vec::new(),
            strip: Strip::new(),
            next: 0,
            address,
            sidebar: false,
            drag: titlebar::DragState::default(),
            _address: blur,
        };
        shell.open(url.map(|url| resolve(&url)), window, cx);
        shell
    }

    fn front(&self) -> Option<&Tab> {
        let id = self.strip.active()?;
        self.tabs.iter().find(|tab| tab.id == *id)
    }

    fn front_page(&self) -> Option<Entity<WebView>> {
        self.front()?.page().cloned()
    }

    fn page(
        &self,
        url: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> (Entity<WebView>, Subscription) {
        let page = cx.new(|cx| WebView::new(url, window, cx));
        let events = cx.subscribe_in(&page, window, |this, page, event, window, cx| {
            if let WebViewEvent::Title(title) = event
                && this.front_page().as_ref() == Some(page)
            {
                window.set_window_title(title);
            }
            cx.notify();
        });
        (page, events)
    }

    /// Opens a tab in front: on `url`, or on the start page with the address
    /// field focused.
    fn open(&mut self, url: Option<String>, window: &mut Window, cx: &mut Context<Self>) {
        let id = self.next;
        self.next += 1;
        let page = url.map(|url| self.page(url, window, cx));
        let focus = page.as_ref().map(|(page, _)| page.focus_handle(cx));
        self.tabs.push(Tab { id, page });
        self.strip.open(id);
        match focus {
            Some(focus) => window.focus(&focus, cx),
            None => self.edit_address(window, cx),
        }
        self.titled(window, cx);
        cx.notify();
    }

    fn close(&mut self, id: u64, window: &mut Window, cx: &mut Context<Self>) {
        self.strip.close(&id);
        self.tabs.retain(|tab| tab.id != id);
        if self.tabs.is_empty() {
            self.open(None, window, cx);
        } else {
            self.shown(window, cx);
        }
    }

    fn show(&mut self, id: u64, window: &mut Window, cx: &mut Context<Self>) {
        self.strip.activate(&id);
        self.shown(window, cx);
    }

    /// After the front tab changed: keys go to its page, or to the window.
    fn shown(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        match self.front_page() {
            Some(page) => window.focus(&page.focus_handle(cx), cx),
            None => window.focus(&self.focus, cx),
        }
        self.titled(window, cx);
        cx.notify();
    }

    fn titled(&self, window: &mut Window, cx: &App) {
        let title = self.front().map(|tab| self.label(tab, cx));
        window.set_window_title(title.as_deref().unwrap_or("Browse"));
    }

    /// Loads `url` in the front tab, which builds its page if it has none.
    fn navigate(&mut self, url: String, window: &mut Window, cx: &mut Context<Self>) {
        let Some(id) = self.strip.active().copied() else {
            return;
        };
        let existing = self.front_page();
        let page = match existing {
            Some(page) => {
                page.update(cx, |page, _| page.load(url));
                page
            }
            None => {
                let (page, events) = self.page(url, window, cx);
                if let Some(tab) = self.tabs.iter_mut().find(|tab| tab.id == id) {
                    tab.page = Some((page.clone(), events));
                }
                page
            }
        };
        window.focus(&page.focus_handle(cx), cx);
        cx.notify();
    }

    fn edit_address(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let url = self
            .front_page()
            .and_then(|page| page.read(cx).location().map(str::to_owned))
            .unwrap_or_default();
        self.address
            .update(cx, |field, cx| field.set_content(url, cx));
        window.focus(&self.address.focus_handle(cx), cx);
        window.dispatch_action(Box::new(SelectAll), cx);
        cx.notify();
    }

    fn editing(&self, window: &Window, cx: &App) -> bool {
        self.address.focus_handle(cx).is_focused(window)
    }

    /// What a tab is called: its page's title, else its host.
    fn label(&self, tab: &Tab, cx: &App) -> String {
        let Some(page) = tab.page() else {
            return "Start Page".into();
        };
        let page = page.read(cx);
        if !page.title().is_empty() {
            return page.title().to_owned();
        }
        page.location().map(host).unwrap_or_default().to_owned()
    }

    fn new_tab(&mut self, _: &NewTab, window: &mut Window, cx: &mut Context<Self>) {
        self.open(None, window, cx);
    }

    fn close_tab(&mut self, _: &CloseTab, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(id) = self.strip.active().copied() {
            self.close(id, window, cx);
        }
    }

    fn next_tab(&mut self, _: &NextTab, window: &mut Window, cx: &mut Context<Self>) {
        self.strip.cycle(1);
        self.shown(window, cx);
    }

    fn previous_tab(&mut self, _: &PreviousTab, window: &mut Window, cx: &mut Context<Self>) {
        self.strip.cycle(-1);
        self.shown(window, cx);
    }

    fn focus_address(&mut self, _: &FocusAddress, window: &mut Window, cx: &mut Context<Self>) {
        self.edit_address(window, cx);
    }

    fn reload(&mut self, _: &Reload, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(page) = self.front_page() {
            page.update(cx, |page, _| page.reload());
        }
    }

    fn back(&mut self, _: &Back, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(page) = self.front_page() {
            page.update(cx, |page, _| page.back());
        }
    }

    fn forward(&mut self, _: &Forward, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(page) = self.front_page() {
            page.update(cx, |page, _| page.forward());
        }
    }

    fn toggle_sidebar(&mut self, _: &ToggleSidebar, _: &mut Window, cx: &mut Context<Self>) {
        self.sidebar = !self.sidebar;
        cx.notify();
    }

    fn go(&mut self, _: &Go, window: &mut Window, cx: &mut Context<Self>) {
        let typed = self.address.read(cx).content().trim().to_owned();
        if !typed.is_empty() {
            self.navigate(resolve(&typed), window, cx);
        }
    }

    fn cancel(&mut self, _: &Cancel, window: &mut Window, cx: &mut Context<Self>) {
        self.shown(window, cx);
    }

    fn toolbar(&self, theme: &Theme, window: &Window, cx: &mut Context<Self>) -> impl IntoElement {
        let button = |key: &'static str, glyph: &'static [u8]| {
            theme
                .icon_button(glyph, ButtonStyle::Ghost, None)
                .id(key)
                .flex_none()
        };
        let lights = if window.is_fullscreen() {
            8.0
        } else {
            LIGHTS_ROOM
        };
        // Both sides grow from zero alike, so the pill sits at the toolbar's
        // centre until one side's buttons need more than half of what is left.
        let side = || {
            div()
                .flex_1()
                .self_stretch()
                .flex()
                .items_center()
                .gap(px(4.0))
        };
        titlebar::titlebar("toolbar", false, window)
            .h(px(Theme::HEADER_HEIGHT))
            .flex_none()
            .gap(px(4.0))
            .bg(theme.surface)
            .when(self.tabs.len() < 2, |bar| {
                bar.border_b_1().border_color(theme.border)
            })
            .child(
                side()
                    .pl(px(lights))
                    .child(
                        button("sidebar", icons::glyph::PanelLeft).on_click(cx.listener(
                            |this, _, window, cx| this.toggle_sidebar(&ToggleSidebar, window, cx),
                        )),
                    )
                    .child(
                        button("back", icons::glyph::ChevronLeft).on_click(
                            cx.listener(|this, _, window, cx| this.back(&Back, window, cx)),
                        ),
                    )
                    .child(button("forward", icons::glyph::ChevronRight).on_click(
                        cx.listener(|this, _, window, cx| this.forward(&Forward, window, cx)),
                    ))
                    .child(titlebar::grip("grip-leading", &self.drag, window)),
            )
            .child(self.address_bar(theme, window, cx))
            .child(
                side()
                    .pr(px(12.0))
                    .child(titlebar::grip("grip-trailing", &self.drag, window))
                    .child(
                        button("new-tab", icons::glyph::Plus).on_click(
                            cx.listener(|this, _, window, cx| this.open(None, window, cx)),
                        ),
                    ),
            )
    }

    fn address_bar(&self, theme: &Theme, window: &Window, cx: &mut Context<Self>) -> AnyElement {
        let editing = self.editing(window, cx);
        let page = self.front_page();
        let location = page
            .as_ref()
            .and_then(|page| page.read(cx).location().map(str::to_owned));
        let loading = page.as_ref().is_some_and(|page| page.read(cx).is_loading());
        let pill = div()
            .id("address")
            .relative()
            .flex_basis(px(ADDRESS_MAX_WIDTH))
            .min_w(px(200.0))
            .max_w(px(ADDRESS_MAX_WIDTH))
            .h(px(ADDRESS_HEIGHT))
            .px(px(10.0))
            .flex()
            .items_center()
            .gap(px(6.0))
            .rounded(px(Theme::BASE_RADIUS))
            .bg(theme.input_bg)
            .text_style(TextStyle::Callout)
            .overflow_hidden();
        let glyph = |glyph: &'static [u8]| {
            ui::icons::icon(glyph)
                .size(px(13.0))
                .flex_none()
                .text_color(theme.text_muted)
        };
        let pill = if editing {
            pill.border_1()
                .border_color(theme.accent)
                .child(glyph(icons::glyph::Search))
                .child(div().flex_1().min_w_0().child(self.address.clone()))
        } else {
            let secure = location
                .as_deref()
                .is_some_and(|url| url.starts_with("https://"));
            let shown = location.as_deref().map(host).unwrap_or_default();
            pill.cursor_text()
                .on_click(cx.listener(|this, _, window, cx| this.edit_address(window, cx)))
                // Balances the reload button, so the host sits at the pill's centre.
                .when(page.is_some(), |pill| {
                    pill.child(div().flex_none().w(px(RELOAD_SIZE)))
                })
                .child(div().flex_1())
                .when(secure, |pill| pill.child(glyph(icons::glyph::Lock)))
                .when(location.is_none(), |pill| {
                    pill.child(glyph(icons::glyph::Search))
                })
                .child(match location {
                    Some(_) => div().truncate().child(SharedString::from(shown.to_owned())),
                    None => div()
                        .text_color(theme.text_muted)
                        .child("Search or enter website name"),
                })
                .child(div().flex_1())
                .when(page.is_some(), |pill| {
                    pill.child(
                        theme
                            .icon_button(icons::glyph::RotateCw, ButtonStyle::Ghost, None)
                            .id("reload")
                            .flex_none()
                            .size(px(RELOAD_SIZE))
                            .on_click(cx.listener(|this, _, window, cx| {
                                cx.stop_propagation();
                                this.reload(&Reload, window, cx)
                            })),
                    )
                })
        };
        pill.when(loading, |pill| {
            pill.child(
                div()
                    .absolute()
                    .left_0()
                    .bottom_0()
                    .h(px(2.0))
                    .w(relative(1.0))
                    .bg(theme.accent),
            )
        })
        .into_any_element()
    }

    fn tab_bar(&self, theme: &Theme, cx: &mut Context<Self>) -> impl IntoElement {
        let front = self.strip.active().copied();
        div()
            .flex_none()
            .h(px(TAB_BAR_HEIGHT))
            .flex()
            .gap(px(4.0))
            .px(px(8.0))
            .pb(px(4.0))
            .bg(theme.surface)
            .border_b_1()
            .border_color(theme.border)
            .children(self.strip.tabs().iter().filter_map(|id| {
                let tab = self.tabs.iter().find(|tab| tab.id == *id)?;
                let id = *id;
                let group = SharedString::from(format!("tab-{id}"));
                let is_front = front == Some(id);
                Some(
                    div()
                        .id(("tab", id))
                        .group(group.clone())
                        .relative()
                        .flex_1()
                        .min_w_0()
                        .h_full()
                        .px(px(28.0))
                        .flex()
                        .items_center()
                        .justify_center()
                        .gap(px(6.0))
                        .rounded(px(Theme::BASE_RADIUS - 2.0))
                        .text_style(TextStyle::Callout)
                        .map(|el| match is_front {
                            true => el.bg(theme.element_active).text_color(theme.text),
                            false => el
                                .text_color(theme.text_muted)
                                .hover(|el| el.bg(theme.element_hover)),
                        })
                        .on_click(cx.listener(move |this, _, window, cx| this.show(id, window, cx)))
                        .child(
                            div()
                                .id(("close", id))
                                .absolute()
                                .left(px(6.0))
                                .p(px(2.0))
                                .rounded(px(4.0))
                                .invisible()
                                .group_hover(group, |el| el.visible())
                                .hover(|el| el.bg(theme.element_hover))
                                .on_click(cx.listener(move |this, _, window, cx| {
                                    cx.stop_propagation();
                                    this.close(id, window, cx)
                                }))
                                .child(
                                    ui::icons::icon(icons::glyph::X)
                                        .size(px(11.0))
                                        .text_color(theme.text_muted),
                                ),
                        )
                        .child(
                            ui::icons::icon(icons::glyph::Globe)
                                .size(px(13.0))
                                .flex_none()
                                .text_color(theme.text_muted),
                        )
                        .child(
                            div()
                                .min_w_0()
                                .truncate()
                                .child(SharedString::from(self.label(tab, cx))),
                        ),
                )
            }))
    }

    fn sidebar(&self, theme: &Theme, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex_none()
            .w(px(SIDEBAR_WIDTH))
            .h_full()
            .p(px(8.0))
            .flex()
            .flex_col()
            .gap(px(2.0))
            .bg(theme.surface)
            .border_r_1()
            .border_color(theme.border)
            .child(
                div()
                    .px(px(8.0))
                    .py(px(4.0))
                    .text_style(TextStyle::Caption)
                    .text_color(theme.text_faint)
                    .child("Favorites"),
            )
            .children(FAVORITES.iter().enumerate().map(|(index, (name, url))| {
                div()
                    .id(("favorite", index))
                    .flex()
                    .items_center()
                    .gap(px(8.0))
                    .px(px(8.0))
                    .h(px(Theme::BUTTON_HEIGHT + 4.0))
                    .rounded(px(Theme::BASE_RADIUS - 2.0))
                    .text_style(TextStyle::Callout)
                    .hover(|el| el.bg(theme.element_hover))
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.navigate((*url).to_owned(), window, cx)
                    }))
                    .child(
                        ui::icons::icon(icons::glyph::Star)
                            .size(px(13.0))
                            .text_color(theme.text_muted),
                    )
                    .child(*name)
            }))
    }

    fn start_page(&self, theme: &Theme, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .flex()
            .justify_center()
            .bg(theme.bg)
            .child(
                div()
                    .w(px(4.0 * (TILE + 40.0)))
                    .pt(px(96.0))
                    .flex()
                    .flex_col()
                    .gap(px(16.0))
                    .child(
                        div()
                            .text_style(TextStyle::Title2)
                            .text_color(theme.text)
                            .child("Favorites"),
                    )
                    .child(div().flex().flex_wrap().gap_y(px(16.0)).children(
                        FAVORITES.iter().enumerate().map(|(index, (name, url))| {
                            let initial = name.chars().next().unwrap_or('?').to_string();
                            div()
                                .id(("tile", index))
                                .w(px(TILE + 40.0))
                                .flex()
                                .flex_col()
                                .items_center()
                                .gap(px(6.0))
                                .cursor_pointer()
                                .on_click(cx.listener(move |this, _, window, cx| {
                                    this.navigate((*url).to_owned(), window, cx)
                                }))
                                .child(
                                    div()
                                        .size(px(TILE))
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .rounded(px(12.0))
                                        .bg(theme.surface_card)
                                        .border_1()
                                        .border_color(theme.border)
                                        .text_style(TextStyle::Title)
                                        .text_color(theme.text_muted)
                                        .hover(|el| el.bg(theme.element_hover))
                                        .child(initial),
                                )
                                .child(
                                    div()
                                        .text_style(TextStyle::Caption)
                                        .text_color(theme.text_muted)
                                        .child(*name),
                                )
                        }),
                    )),
            )
    }
}

impl Focusable for Shell {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Render for Shell {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::of(cx).clone();
        let content = match self.front_page() {
            Some(page) => div().size_full().child(page).into_any_element(),
            None => self.start_page(&theme, cx).into_any_element(),
        };
        div()
            .key_context(CONTEXT)
            .track_focus(&self.focus)
            .on_action(cx.listener(Self::new_tab))
            .on_action(cx.listener(Self::close_tab))
            .on_action(cx.listener(Self::next_tab))
            .on_action(cx.listener(Self::previous_tab))
            .on_action(cx.listener(Self::focus_address))
            .on_action(cx.listener(Self::reload))
            .on_action(cx.listener(Self::back))
            .on_action(cx.listener(Self::forward))
            .on_action(cx.listener(Self::toggle_sidebar))
            .on_action(cx.listener(Self::go))
            .on_action(cx.listener(Self::cancel))
            .size_full()
            .flex()
            .flex_col()
            .bg(theme.bg)
            .text_color(theme.text)
            .child(self.toolbar(&theme, window, cx))
            .when(self.tabs.len() > 1, |root| {
                root.child(self.tab_bar(&theme, cx))
            })
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .when(self.sidebar, |row| row.child(self.sidebar(&theme, cx)))
                    .child(div().flex_1().min_w_0().h_full().child(content)),
            )
    }
}

/// What the address field shows for `url` while it is not being edited.
fn host(url: &str) -> &str {
    let rest = url.split_once("://").map_or(url, |(_, rest)| rest);
    let host = rest.split(['/', '?', '#']).next().unwrap_or(rest);
    host.strip_prefix("www.").unwrap_or(host)
}

/// What typing `typed` into the address field loads.
fn resolve(typed: &str) -> String {
    if typed.contains("://") {
        typed.to_owned()
    } else if !typed.contains(' ') && typed.contains('.') {
        format!("https://{typed}")
    } else {
        let mut query = String::new();
        for byte in typed.bytes() {
            match byte {
                b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                    query.push(byte as char)
                }
                b' ' => query.push('+'),
                _ => query.push_str(&format!("%{byte:02X}")),
            }
        }
        format!("{SEARCH}{query}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn host_drops_the_scheme_path_and_www() {
        assert_eq!(host("https://www.rust-lang.org/learn?x=1"), "rust-lang.org");
        assert_eq!(host("about:blank"), "about:blank");
    }

    #[test]
    fn typed_text_resolves_to_a_url_or_a_search() {
        assert_eq!(resolve("example.com"), "https://example.com");
        assert_eq!(resolve("http://a.b/c"), "http://a.b/c");
        assert_eq!(resolve("rust gpui"), format!("{SEARCH}rust+gpui"));
        assert_eq!(resolve("c++"), format!("{SEARCH}c%2B%2B"));
    }
}
