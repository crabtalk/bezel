//! The element inspector — `cmd-alt-i`, then click anything.
//!
//! gpui carries the whole mechanism in any debug build (`cfg(debug_assertions)`,
//! no feature needed): pick mode, the hitbox registry, the side panel slot, and
//! a per-element [`DivInspectorState`] holding its style, bounds and content
//! size. All an app supplies is the panel, so this file is just a renderer —
//! written in bezel's own components, which makes it the library inspecting
//! itself.
//!
//! What it answers, in order of how often you want it:
//!
//! 1. **Where is this written?** `source_location` is the file and line the
//!    element was constructed at. For a library whose law is "customisation is
//!    editing the source", that is the whole documentation.
//! 2. **How big is it, really?** `bounds` is the laid-out rectangle and picking
//!    runs off the *hitbox* — so an element whose clickable area disagrees with
//!    what it paints shows up here rather than being guessed at.
//! 3. **What is it?** The element id path, when a component is on screen twice.

use bezel_theme::Theme;
use bezel_ui::{popover, widgets};
use gpui::{
    AnyElement, App, Context, DivInspectorState, InspectorElementId, IntoElement, SharedString,
    Window, div, prelude::*, px,
};

/// Install the inspector panel. Call once at startup, alongside the other
/// `init`s. The toggle itself is an action on the gallery view — see
/// `Gallery::toggle_inspector`.
pub fn init(cx: &mut App) {
    cx.register_inspector_element(|_id, state: &DivInspectorState, _window, cx: &mut App| {
        let theme = Theme::of(cx).clone();
        measurements(&theme, state)
    });
    cx.set_inspector_renderer(Box::new(render));
}

fn render(
    inspector: &mut gpui::Inspector,
    window: &mut Window,
    cx: &mut Context<gpui::Inspector>,
) -> AnyElement {
    let theme = Theme::of(cx).clone();
    let picking = inspector.is_picking();
    let id = inspector.active_element_id().cloned();

    div()
        .size_full()
        .flex()
        .flex_col()
        .bg(theme.surface)
        .border_l_1()
        .border_color(theme.border)
        .font_family(theme.font_sans.clone())
        .text_color(theme.text)
        .text_size(px(13.0))
        .child(
            div()
                .flex_none()
                .h(px(Theme::HEADER_HEIGHT))
                .px(px(12.0))
                .flex()
                .flex_row()
                .items_center()
                .justify_between()
                .border_b_1()
                .border_color(theme.border)
                .child(
                    div()
                        .text_size(px(13.0))
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .child("Inspector"),
                )
                .child(
                    div()
                        .id("inspector-pick")
                        .on_click(cx.listener(|inspector, _, window, _| {
                            inspector.start_picking();
                            window.refresh();
                        }))
                        .child(
                            widgets::toggle_group(&theme).child(widgets::toggle_group_item(
                                &theme,
                                if picking { "Picking…" } else { "Pick" },
                                picking,
                            )),
                        ),
                ),
        )
        .child(
            div()
                .id("inspector-body")
                .flex_1()
                .min_h_0()
                .overflow_y_scroll()
                .p(px(12.0))
                .flex()
                .flex_col()
                .gap(px(12.0))
                .when_some(id, |body, id| body.child(identity(&theme, &id)))
                .children(inspector.render_inspector_states(window, cx))
                .when(!picking, |body| {
                    body.child(
                        div()
                            .text_size(px(12.0))
                            .text_color(theme.text_faint)
                            .child("Press Pick, then click any element."),
                    )
                }),
        )
        .into_any_element()
}

/// Where the element was constructed, and which instance it is.
fn identity(theme: &Theme, id: &InspectorElementId) -> AnyElement {
    let location = id.path.source_location;
    // Absolute paths turn up for some elements; the tail is what is readable
    // and is still enough to open the file.
    let file = location.file();
    let file = file.rsplit_once("bezel/").map_or(file, |(_, tail)| tail);

    field_block(
        theme,
        "Source",
        vec![
            (
                "location",
                format!("{file}:{}:{}", location.line(), location.column()),
            ),
            ("element", format!("{}", id.path.global_id)),
            ("instance", format!("{}", id.instance_id)),
        ],
    )
}

/// Laid-out geometry. `bounds` is what the element occupies; picking runs off
/// its hitbox, so this is where a clickable area that disagrees with the paint
/// becomes visible instead of theoretical.
fn measurements(theme: &Theme, state: &DivInspectorState) -> AnyElement {
    let bounds = state.bounds;
    let content = state.content_size;
    field_block(
        theme,
        "Layout",
        vec![
            (
                "origin",
                format!(
                    "{:.1}, {:.1}",
                    f32::from(bounds.origin.x),
                    f32::from(bounds.origin.y)
                ),
            ),
            (
                "size",
                format!(
                    "{:.1} × {:.1}",
                    f32::from(bounds.size.width),
                    f32::from(bounds.size.height)
                ),
            ),
            (
                "content",
                format!(
                    "{:.1} × {:.1}",
                    f32::from(content.width),
                    f32::from(content.height)
                ),
            ),
        ],
    )
}

/// A titled block of `name: value` rows, values in mono.
fn field_block(
    theme: &Theme,
    title: &'static str,
    rows: Vec<(&'static str, String)>,
) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .gap(px(4.0))
        .child(popover::menu_heading(theme, title))
        .children(rows.into_iter().map(|(name, value)| {
            div()
                .flex()
                .flex_row()
                .items_baseline()
                .gap(px(8.0))
                .child(
                    div()
                        .w(px(64.0))
                        .flex_none()
                        .text_size(px(11.0))
                        .text_color(theme.text_faint)
                        .child(SharedString::from(name)),
                )
                .child(
                    div()
                        .min_w_0()
                        .text_size(px(11.5))
                        .font_family(theme.font_mono.clone())
                        .text_color(theme.text_muted)
                        .child(SharedString::from(value)),
                )
        }))
        .into_any_element()
}
