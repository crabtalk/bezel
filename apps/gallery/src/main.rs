//! The bezel gallery — every component rendered in a real window. This is the
//! dev surface: new components land here the day they land in `crates/ui`.

use bezel_theme::{Theme, appearance};
use bezel_ui::input::TextField;
use bezel_ui::{icons, input, loaders, popover, widgets};
use gpui::{
    App, Bounds, Context, Entity, Focusable, SharedString, Window, WindowBounds, WindowOptions,
    div, prelude::*, px, size,
};

fn main() {
    gpui_platform::application()
        .with_assets(icons::Assets)
        .run(|cx: &mut App| {
            if let Err(err) = bezel_ui::register_fonts(cx) {
                eprintln!("FONT REGISTRATION FAILED: {err:?}");
            }
            appearance::init(appearance::AppearanceMode::System, cx);
            input::init(cx);
            let bounds = Bounds::centered(None, size(px(960.0), px(760.0)), cx);
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    // Glass needs a blurred window background to blur INTO;
                    // without it `material` has nothing behind it.
                    window_background: Theme::of(cx).window_background_appearance(),
                    ..Default::default()
                },
                |window, cx| {
                    appearance::observe_window(window, cx).detach();
                    let gallery = cx.new(|cx| Gallery {
                        search: cx
                            .new(|cx| TextField::new(cx).with_placeholder("Search components…")),
                        filled: cx.new(|cx| {
                            let mut field = TextField::new(cx);
                            field.set_content("Select me with shift-left", cx);
                            field
                        }),
                    });
                    // Focus a field on launch so the caret is visible.
                    let focus = gallery.read(cx).search.focus_handle(cx);
                    window.focus(&focus, cx);
                    gallery
                },
            )
            .unwrap();
            cx.activate(true);
        });
}

struct Gallery {
    search: Entity<TextField>,
    filled: Entity<TextField>,
}

fn section(theme: &Theme, title: &str) -> gpui::Div {
    div().flex().flex_col().gap(px(12.0)).child(
        div()
            .text_size(px(11.0))
            .font_weight(gpui::FontWeight::MEDIUM)
            .text_color(theme.text_faint)
            .child(SharedString::from(popover::tracked_upper(title))),
    )
}

fn row() -> gpui::Div {
    div().flex().flex_row().items_center().gap(px(12.0))
}

impl Render for Gallery {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::of(cx).clone();
        let view = cx.entity_id();

        let buttons = section(&theme, "Buttons").child(
            row()
                .child(popover::button(&theme, "Ghost", "g-ghost"))
                .child(popover::button_prominent(&theme, "Prominent"))
                .child(popover::button_destructive(&theme, "Destructive")),
        );

        let toggles = section(&theme, "Toggle & badges").child(
            row()
                .child(widgets::toggle(&theme, true))
                .child(widgets::toggle(&theme, false))
                .child(widgets::badge(&theme, "badge"))
                .child(widgets::badge_active(&theme, "active")),
        );

        let fields = section(&theme, "Text field").child(
            div()
                .w(px(320.0))
                .flex()
                .flex_col()
                .gap(px(10.0))
                .child(self.search.clone())
                .child(self.filled.clone()),
        );

        let controls = section(&theme, "Checkbox, radio, avatar").child(
            row()
                .child(widgets::checkbox(&theme, true))
                .child(widgets::checkbox(&theme, false))
                .child(widgets::radio_button(&theme, true))
                .child(widgets::radio_button(&theme, false))
                .child(widgets::avatar(&theme, "TC"))
                .child(widgets::avatar(&theme, "K")),
        );

        let tracks = section(&theme, "Progress & slider").child(
            div()
                .w(px(280.0))
                .flex()
                .flex_col()
                .gap(px(16.0))
                .child(widgets::progress_bar(&theme, 0.35))
                .child(widgets::progress_bar(&theme, 0.8))
                .child(widgets::slider(&theme, 0.5)),
        );

        let tabs = section(&theme, "Tabs").child(
            widgets::tab_bar(&theme)
                .child(widgets::tab(&theme, "Components", true))
                .child(widgets::tab(&theme, "Tokens", false))
                .child(widgets::tab(&theme, "Motion", false)),
        );

        let menu = section(&theme, "Menu").child(
            popover::popover_card(&theme).w(px(240.0)).children([
                popover::menu_heading(&theme, "Section").into_any_element(),
                popover::menu_row(&theme, false, "m-one")
                    .child("First item")
                    .into_any_element(),
                popover::menu_row(&theme, true, "m-two")
                    .child("Active item")
                    .into_any_element(),
                popover::divider().into_any_element(),
                popover::menu_row(&theme, false, "m-three")
                    .child("Third item")
                    .into_any_element(),
            ]),
        );

        let group = section(&theme, "Group box").child(
            widgets::group_box(&theme)
                .child(
                    widgets::card_row(&theme, true)
                        .child(widgets::row_tile(&theme, icons::MONITOR))
                        .child(widgets::row_title(&theme, "First row")),
                )
                .child(
                    widgets::card_row(&theme, false)
                        .child(widgets::row_tile(&theme, icons::FOLDER))
                        .child(widgets::row_title(&theme, "Second row")),
                ),
        );

        let spinners = section(&theme, "Loaders").child(
            row()
                .child(loaders::pulse_loader("g-pulse", &theme, 8.0, view, cx))
                .child(loaders::gradient_spinner("g-spin", &theme, 5.0, view, cx))
                .child(loaders::mini_gradient_spinner("g-mini", 2.5, view, cx))
                .child(loaders::loading_word(&theme)),
        );

        // Exercises the fork's backdrop-blur primitive: the card blurs the
        // striped band painted behind it.
        let material = section(&theme, "Material").child(
            div()
                .relative()
                .w(px(420.0))
                .h(px(150.0))
                .child(
                    div()
                        .absolute()
                        .inset_0()
                        .flex()
                        .flex_row()
                        .children((0..14).map(|i| {
                            div().w(px(30.0)).h_full().bg(if i % 2 == 0 {
                                theme.accent
                            } else {
                                theme.warning
                            })
                        })),
                )
                .child(
                    div().absolute().top(px(28.0)).left(px(60.0)).child(
                        bezel_ui::material::material(
                            12.0,
                            bezel_ui::material::MENU_BLUR,
                            popover::popover_card(&theme)
                                .w(px(220.0))
                                .child(popover::menu_row(&theme, false, "mat-a").child("Blurred")),
                        ),
                    ),
                ),
        );

        let strips = section(&theme, "Strips & redacted")
            .child(widgets::error_strip(&theme, "Something went wrong."))
            .child(widgets::warning_strip(&theme, "Heads up, check this."))
            .child(popover::redacted_rows("g-redacted", &theme, 3, view, cx));

        div()
            .id("gallery-scroll")
            .size_full()
            .overflow_y_scroll()
            .bg(theme.bg)
            .font_family(theme.font_sans.clone())
            .text_color(theme.text)
            .text_size(px(14.0))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(28.0))
                    .p(px(32.0))
                    .child(
                        div()
                            .text_size(px(18.0))
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .child("bezel gallery"),
                    )
                    .child(buttons)
                    .child(fields)
                    .child(toggles)
                    .child(controls)
                    .child(tracks)
                    .child(tabs)
                    .child(menu)
                    .child(group)
                    .child(spinners)
                    .child(material)
                    .child(strips),
            )
    }
}
