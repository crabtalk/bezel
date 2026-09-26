//! Each section's body, by group, and the helpers they compose from.

use crate::*;

mod controls;
mod data;
mod foundations;
mod material;
mod navigation;
mod overlays;
mod patterns;

/// The gallery's own rhythm, looser than the system gap the library defaults to.
pub(crate) const GALLERY_RHYTHM: f32 = 12.0;

/// The vertical rhythm every page body uses — the gallery's own, said once
/// here the way `VStack(spacing: 12)` says it.
pub(crate) fn stack() -> gpui::Div {
    ui::stack::column().gap(px(GALLERY_RHYTHM))
}

/// How far the body size runs: macOS's smallest UI size (`labelFontSize 10`,
/// which is the ladder's own floor) up to where `STATUS_STRIP_HEIGHT` is exactly
/// the [`TextStyle::Callout`] it holds and the tightest chrome in the library
/// has definitively failed.
pub(crate) const BASE_TEXT_RANGE: (f32, f32) = (
    TextStyle::Caption2.size(),
    Theme::STATUS_STRIP_HEIGHT * TextStyle::Body.size() / TextStyle::Callout.size(),
);

/// Every role on the ladder, largest first.
pub(crate) const TYPE_SCALE: &[TextStyle] = &[
    TextStyle::LargeTitle,
    TextStyle::Title,
    TextStyle::Title2,
    TextStyle::Title3,
    TextStyle::Headline,
    TextStyle::Body,
    TextStyle::Callout,
    TextStyle::Subheadline,
    TextStyle::Footnote,
    TextStyle::Caption,
    TextStyle::Caption2,
];

/// Every named spec in `motion`.
pub(crate) const MOTION_CATALOG: &[(&str, motion::MotionSpec)] = &[
    ("FADE_IN", motion::FADE_IN),
    ("FADE_QUICK", motion::FADE_QUICK),
    ("MENU_IN", motion::MENU_IN),
    ("MENU_OUT", motion::MENU_OUT),
    ("DIALOG_IN", motion::DIALOG_IN),
    ("SPLASH_OUT", motion::SPLASH_OUT),
    ("RESIZE", motion::RESIZE),
    ("TAB_SLIDE", motion::TAB_SLIDE),
    ("COLLAPSE", motion::COLLAPSE),
    ("CHEVRON", motion::CHEVRON),
    ("SCROLL_GLIDE", motion::SCROLL_GLIDE),
    ("HOVER_FADE", motion::HOVER_FADE),
    ("PULSE", motion::PULSE),
    ("GRADIENT_SPIN", motion::GRADIENT_SPIN),
];

/// The colour tokens, by role. Hand-listed because `Theme` is a plain struct —
/// there is no reflection, and a token that never reaches this list is a token
/// nobody can find.
pub(crate) fn color_groups(theme: &Theme) -> Vec<(&'static str, Vec<(&'static str, gpui::Hsla)>)> {
    vec![
        (
            "Surfaces",
            vec![
                ("bg", theme.bg),
                ("surface", theme.surface),
                ("surface_raised", theme.surface_raised),
                ("surface_card", theme.surface_card),
                ("surface_dialog", theme.surface_dialog),
                ("surface_overlay", theme.surface_overlay),
                ("band", theme.band),
                ("input_bg", theme.input_bg),
            ],
        ),
        (
            "Text",
            vec![
                ("text", theme.text),
                ("text_muted", theme.text_muted),
                ("text_faint", theme.text_faint),
                ("text_dim", theme.text_dim),
                ("on_solid", theme.on_solid),
                ("on_accent", theme.on_accent),
            ],
        ),
        (
            "Lines & fills",
            vec![
                ("border", theme.border),
                ("border_strong", theme.border_strong),
                ("element_hover", theme.element_hover),
                ("element_active", theme.element_active),
                ("selection", theme.selection),
                ("caret", theme.caret),
                ("ring", theme.ring),
                ("solid", theme.solid),
            ],
        ),
        (
            "Accent & status",
            vec![
                ("accent", theme.accent),
                ("accent_strong", theme.accent_strong),
                ("success", theme.success),
                ("success_muted", theme.success_muted),
                ("warning", theme.warning),
                ("warning_muted", theme.warning_muted),
                ("danger", theme.danger),
                ("danger_muted", theme.danger_muted),
                ("danger_strong", theme.danger_strong),
                ("busy", theme.busy),
            ],
        ),
        (
            "Code & diff",
            vec![
                ("code_text", theme.code_text),
                ("code_wash", theme.code_wash),
                ("diff_add", theme.diff_add),
                ("diff_del", theme.diff_del),
                ("diff_hunk_bg", theme.diff_hunk_bg),
            ],
        ),
    ]
}

/// One colour chip: the paint over the page background, its token name, and the
/// contrast it lands at — the number that decides whether text on it is legible.
pub(crate) fn swatch(theme: &Theme, name: &'static str, color: gpui::Hsla) -> gpui::Div {
    div()
        .w(px(124.0))
        .flex()
        .flex_col()
        .gap(px(5.0))
        .child(
            div()
                .h(px(44.0))
                .w_full()
                .rounded(px(Theme::control_radius()))
                .border_1()
                .border_color(theme.border)
                .bg(color),
        )
        .child(
            div()
                .text_style(TextStyle::Caption)
                .font_family(theme.font_mono.clone())
                .text_color(theme.text_muted)
                .child(SharedString::from(name)),
        )
        .child(
            div()
                .text_style(TextStyle::Caption)
                .text_color(theme.text_faint)
                .child(SharedString::from(format!(
                    "{:.1}:1",
                    theme::contrast_ratio(color, theme.bg)
                ))),
        )
}

/// A constant drawn at its own size, so a number reads as a distance.
pub(crate) fn measure(theme: &Theme, name: &'static str, value: f32) -> gpui::Div {
    div()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(12.0))
        .child(
            div()
                .w(px(170.0))
                .flex_none()
                .text_style(TextStyle::Subheadline)
                .font_family(theme.font_mono.clone())
                .text_color(theme.text_muted)
                .child(SharedString::from(name)),
        )
        .child(
            div()
                .h(px(10.0))
                .w(px(value))
                .rounded(px(2.0))
                .bg(theme.accent),
        )
        .child(
            div()
                .text_style(TextStyle::Subheadline)
                .text_color(theme.text_faint)
                .child(SharedString::from(format!("{value}"))),
        )
}

/// An easing curve as a bar chart of its own output. gpui has no path drawing
/// at this rev, and sampling the real function beats drawing an approximation
/// of it.
pub(crate) fn curve_plot(theme: &Theme, name: &str, at: impl Fn(f32) -> f32) -> gpui::Div {
    const SAMPLES: usize = 28;
    const HEIGHT: f32 = 40.0;
    let plot = div()
        .flex()
        .flex_row()
        .items_end()
        .gap(px(1.0))
        .h(px(HEIGHT))
        .children((0..SAMPLES).map(|i| {
            let t = i as f32 / (SAMPLES - 1) as f32;
            div()
                .w(px(3.0))
                .h(px((at(t).clamp(0.0, 1.0) * HEIGHT).max(1.0)))
                .rounded(px(1.0))
                .bg(theme.accent)
        }));
    if name.is_empty() {
        plot
    } else {
        div().flex().flex_col().gap(px(6.0)).child(plot).child(
            div()
                .text_style(TextStyle::Subheadline)
                .font_family(theme.font_mono.clone())
                .text_color(theme.text_faint)
                .child(SharedString::from(name.to_string())),
        )
    }
}

/// A font family shown in itself, named beside it.
pub(crate) fn type_row(theme: &Theme, family: SharedString, name: &'static str) -> gpui::Div {
    div()
        .flex()
        .flex_row()
        .items_baseline()
        .gap(px(12.0))
        .child(
            div()
                .w(px(90.0))
                .flex_none()
                .text_style(TextStyle::Subheadline)
                .font_family(theme.font_mono.clone())
                .text_color(theme.text_faint)
                .child(SharedString::from(name)),
        )
        .child(
            div()
                .text_style(TextStyle::Title3)
                .font_family(family.clone())
                .child(family),
        )
}

/// One muted line telling you how to try a component whose whole behaviour is
/// an interaction — the page would otherwise look like a dead button.
pub(crate) fn hint(theme: &Theme, copy: &str) -> gpui::Div {
    div()
        .text_style(TextStyle::Callout)
        .text_color(theme.text_muted)
        .child(SharedString::from(copy.to_string()))
}

/// One field captioned with the [`Shape`] that produced it, so the numbers on
/// the page read as this example's arguments rather than as library defaults.
pub(crate) fn shape_demo(
    theme: &Theme,
    shape: &'static str,
    field: Entity<TextField>,
) -> gpui::Div {
    stack()
        .gap(px(6.0))
        .child(
            div()
                .text_style(TextStyle::Subheadline)
                .font_family(theme.font_mono.clone())
                .text_color(theme.text_faint)
                .child(SharedString::from(shape)),
        )
        .child(field)
}

/// The page for a [`planned`] component: why it is not here, and what the work
/// actually is. Numbered rather than bulleted — the entries are the order the
/// work happens in, not a set of features.
///
/// `work` is empty wherever nothing has been designed yet. That is the honest
/// answer, and an empty list is itself the measurement.
///
/// Unused for the same reason [`planned`] is: nothing in the catalog is unbuilt
/// at the moment. Kept for the next thing that is.
#[allow(dead_code)]
pub(crate) fn todo(
    theme: &Theme,
    status: &str,
    summary: &str,
    work: &[&'static str],
) -> AnyElement {
    let amber = theme.warning;
    stack()
        .child(
            div()
                .self_start()
                .px(px(7.0))
                .py(px(2.0))
                .rounded(px(5.0))
                .border_1()
                .border_color(amber.opacity(0.2))
                .bg(amber.opacity(0.06))
                .text_style(TextStyle::Caption2)
                .text_color(theme.warning_muted.opacity(0.9))
                .child(SharedString::from(popover::tracked_upper(status))),
        )
        .child(
            div()
                .text_style(TextStyle::Body)
                .text_color(theme.text_muted)
                .child(SharedString::from(summary.to_string())),
        )
        .when(!work.is_empty(), |page| {
            page.child(
                theme
                    .group_box()
                    .children(work.iter().enumerate().map(|(index, step)| {
                        theme
                            .card_row(index == 0)
                            .hover(|s| s.bg(theme.element_hover))
                            .items_start()
                            .child(
                                div()
                                    .flex_none()
                                    .w(px(12.0))
                                    .text_style(TextStyle::Callout)
                                    .font_family(theme.font_mono.clone())
                                    .text_color(theme.text_faint)
                                    .child(SharedString::from(format!("{}", index + 1))),
                            )
                            .child(
                                div()
                                    .text_style(TextStyle::Callout)
                                    .text_color(theme.text_muted)
                                    .child(SharedString::from(*step)),
                            )
                            .into_any_element()
                    })),
            )
        })
        .into_any_element()
}

/// Wire a focusable control to click *and* to `enter`/`space`, from one closure.
///
/// [`focus::Activate`] is dispatched rather than folded into `on_click` because
/// only the caller knows what a press means — which makes two call sites per
/// control, and two call sites are where a keyboard affordance quietly starts
/// doing something else than the mouse. Taking the behaviour once removes the
/// chance.
pub(crate) fn pressable<T: 'static>(
    el: gpui::Div,
    id: impl Into<gpui::ElementId>,
    cx: &Context<T>,
    press: impl Fn(&mut T, &mut Context<T>) + Clone + 'static,
) -> gpui::Stateful<gpui::Div> {
    let by_key = press.clone();
    el.id(id)
        .on_click(cx.listener(move |view, _, _, cx| press(view, cx)))
        .on_action(cx.listener(move |view, _: &focus::Activate, _, cx| by_key(view, cx)))
}

/// Bytes in the shortest unit that keeps them under four digits — the kind of
/// formatting a right-aligned column exists for.
pub(crate) fn format_size(bytes: u32) -> String {
    match bytes {
        0..1_000 => format!("{bytes} B"),
        1_000..1_000_000 => format!("{:.1} kB", bytes as f32 / 1_000.0),
        _ => format!("{:.1} MB", bytes as f32 / 1_000_000.0),
    }
}

/// Today, locally — the one thing [`Calendar`] asks its host for.
///
/// This is the boundary conversion, and the reason it is worth showing: bezel
/// carries no clock and no chrono, so the app that has both hands over three
/// numbers and keeps its date library to itself.
pub(crate) fn today() -> Date {
    use chrono::Datelike as _;
    let now = chrono::Local::now().date_naive();
    Date::new(now.year(), now.month() as u8, now.day() as u8).expect("chrono deals in real dates")
}

pub(crate) fn row() -> gpui::Div {
    ui::stack::row().gap(px(GALLERY_RHYTHM))
}

pub(crate) fn column() -> gpui::Div {
    div()
        .w_full()
        .max_w(px(COLUMN_WIDTH))
        .flex()
        .flex_col()
        .gap(px(28.0))
}
