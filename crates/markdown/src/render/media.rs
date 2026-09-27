//! Images and bookmark cards.

use super::*;

/// A picture and the caption under it, which is the alt text a caret can reach.
///
/// The caption row appears when there is something to read or somewhere to
/// type, so a document being read is not a column of pictures each trailing a
/// blank line. With no URL yet the picture is a dashed row instead — the shape
/// the slash menu makes, waiting to be told what to show.
pub(super) fn image(
    url: &str,
    alt: &Text,
    width: Option<u32>,
    overlay: Overlay,
    typography: &Typography,
    theme: &Theme,
    cx: &App,
) -> AnyElement {
    let hint = SharedString::new_static(CAPTION_HINT);
    let overlay = Overlay {
        placeholder: Some(&hint),
        ..overlay.at(Part::Caption)
    };
    let picture = if url.is_empty() {
        div()
            .h(px(IMAGE_EMPTY_HEIGHT))
            .flex()
            .items_center()
            .px(px(CARD_PADDING))
            .rounded(px(Theme::button_radius()))
            .border_1()
            .border_dashed()
            .border_color(theme.border)
            .text_size(px(typography.body.size()))
            .text_color(theme.text_muted)
            .child(IMAGE_EMPTY)
    } else {
        let picture = img(image_source(url, overlay.base));
        let box_ = div()
            .relative()
            .rounded(px(Theme::button_radius()))
            .overflow_hidden()
            .border_1()
            .border_color(theme.border)
            .children(overlay.layouts.map(|layouts| {
                let layouts = layouts.clone();
                let ix = overlay.block;
                canvas(
                    move |bounds, _, _| layouts.record_picture(ix, bounds),
                    |_, _, _, _| (),
                )
                .absolute()
                .size_full()
            }));
        match width {
            // A stated width is the box's: it hugs, so the border is around
            // the picture rather than around the column beside it, and the
            // picture fills what the box settled on — which `max_w_full`
            // holds inside the page however wide the width was written.
            Some(width) => box_
                .self_start()
                .max_w_full()
                .w(px(width as f32))
                .child(picture.w(px(width as f32)).max_w_full()),
            // Unstated, the picture scales itself against the column, which
            // is a percentage and so needs a box that spans one to measure.
            None => box_.child(picture.max_w_full()),
        }
    };
    div()
        .flex()
        .flex_col()
        .gap(px(CAPTION_GAP))
        .child(picture)
        // An empty caption still paints while the caret is in it, or there
        // would be nothing to type into and no hint saying so.
        .when(
            overlay.caption == Caption::Shown && (!alt.is_empty() || overlay.caret().is_some()),
            |el| {
                el.child(text_element(
                    alt,
                    typography.caption.size(),
                    typography.caption.line_height(),
                    FontWeight::NORMAL,
                    overlay,
                    theme,
                    cx,
                ))
            },
        )
        .into_any_element()
}

/// A bookmark, in Notion's proportions: a fixed-height row with the text on the
/// left and an image panel of a fixed width on the right, all of it one click
/// target. [`Form::Embed`] turns the row into a column and gives the image the
/// card's full width instead, and [`Form::Chip`] is neither — a pill of favicon
/// and title, which is what an inline mention would be if shaped text had
/// anywhere to put a picture.
///
/// The row is a fixed height with its footer pinned to the bottom, because a
/// preview resolves *after* the card has painted — a blurb arriving into a box
/// that grows would shove every block below it down the page. An embed's cover
/// holds that height, so its text hugs.
pub(super) fn bookmark(
    ix: usize,
    url: &str,
    form: Form,
    typography: &Typography,
    theme: &Theme,
    cx: &App,
) -> AnyElement {
    let preview = preview::of(cx, url).unwrap_or_default();
    let host = SharedString::from(preview::host(url).to_string());
    let label = preview.label.clone().unwrap_or_else(|| host.clone());
    let title = preview
        .title
        .clone()
        .unwrap_or_else(|| SharedString::from(url.to_string()));

    // Owned, because the image panel's fallback outlives this call: gpui asks
    // for the replacement element only once the fetch has failed.
    let (icon, muted, wash) = (preview.icon.clone(), theme.text_muted, theme.element_hover);
    let site = host.clone();
    let mark = move |size: f32| {
        let host = site.clone();
        match icon.clone() {
            Some(icon) => img(icon)
                .size(px(size))
                .rounded(px(size / 4.0))
                .with_fallback(move || initial(&host, size, muted, wash))
                .into_any_element(),
            None => initial(&host, size, muted, wash),
        }
    };

    if form == Form::Chip {
        let open = url.to_string();
        let pill = div()
            .id(ElementId::named_usize("md-chip", ix))
            .flex()
            .flex_row()
            .items_center()
            .gap(px(6.0))
            .px(px(CHIP_BLOCK_PAD_X))
            .py(px(CHIP_BLOCK_PAD_Y))
            .rounded(px(Theme::control_radius()))
            .border_1()
            .border_color(theme.border)
            .bg(theme.element_hover)
            .text_size(px(typography.body.size()))
            .line_height(px(typography.body.line_height()))
            .text_color(theme.text)
            .cursor(CursorStyle::PointingHand)
            .hover(|el| el.bg(theme.element_active))
            .on_click(move |_, _, cx| cx.open_url(&open))
            .child(mark(CHIP_ICON))
            // The host, not the URL, when nothing has resolved it: a chip is
            // the short form, and a raw URL in a pill is the long one.
            .child(
                div()
                    .min_w_0()
                    .truncate()
                    .child(preview.title.unwrap_or(label)),
            );
        // A block's own box is `display: block`, where a pill would take the
        // whole width. One flex row around it is what lets it hug its label.
        return div().flex().flex_row().child(pill).into_any_element();
    }

    let words = div()
        .flex()
        .flex_col()
        .min_w_0()
        .px(px(CARD_PADDING))
        .py(px(CARD_PADDING - 2.0))
        .child(
            div()
                .truncate()
                .text_size(px(typography.body.size()))
                .line_height(px(typography.body.line_height()))
                .text_color(theme.text)
                .child(title),
        )
        .children(preview.description.map(|blurb| {
            div()
                .line_clamp(2)
                .text_size(px(typography.card.size()))
                .line_height(px(typography.card.line_height()))
                .text_color(theme.text_muted)
                .child(blurb)
        }))
        .child(
            div()
                .mt_auto()
                .pt(px(6.0))
                .flex()
                .items_center()
                .gap(px(6.0))
                .text_size(px(typography.card.size()))
                .text_color(theme.text_muted)
                .child(mark(CARD_ICON))
                .child(div().truncate().child(label)),
        );

    let picture = corners(div(), form)
        .bg(theme.surface)
        .flex()
        .items_center()
        .justify_center()
        .overflow_hidden()
        .child(match preview.image {
            Some(image) => corners(img(image).size_full().object_fit(ObjectFit::Cover), form)
                .with_fallback(move || mark(CARD_COVER))
                .into_any_element(),
            None => mark(CARD_COVER),
        });

    let open = url.to_string();
    let card = div()
        .id(ElementId::named_usize("md-bookmark", ix))
        .flex()
        .w_full()
        .overflow_hidden()
        .rounded(px(Theme::button_radius()))
        .border(px(CARD_BORDER))
        .border_color(theme.border)
        .bg(theme.surface_card)
        .cursor(CursorStyle::PointingHand)
        .hover(|el| el.bg(theme.element_hover))
        .on_click(move |_, _, cx| cx.open_url(&open));

    if form == Form::Embed {
        card.flex_col()
            .child(picture.w_full().h(px(CARD_COVER_HEIGHT)))
            .child(words.w_full())
    } else {
        card.h(px(CARD_HEIGHT))
            .child(words.flex_1())
            .child(picture.flex_none().w(px(CARD_IMAGE_WIDTH)).h_full())
    }
    .into_any_element()
}

/// The card's corners, on the panel that reaches them: a content mask is a
/// rectangle, so a picture paints square over a rounded card unless it carries
/// the radius itself, concentric inside the card's border.
pub(super) fn corners<T: Styled>(element: T, form: Form) -> T {
    let corner = px(Theme::inset_radius(Theme::button_radius(), CARD_BORDER));
    match form {
        Form::Embed => element.rounded_t(corner),
        _ => element.rounded_r(corner),
    }
}

/// The mark a site gets before anyone has fetched its favicon: its host's first
/// letter, which is a placeholder no icon set has to ship.
pub(super) fn initial(host: &str, size: f32, color: Hsla, wash: Hsla) -> AnyElement {
    div()
        .flex_none()
        .size(px(size))
        .rounded(px(size / 4.0))
        .bg(wash)
        .flex()
        .items_center()
        .justify_center()
        .text_size(px(size * 0.55))
        .text_color(color)
        .child(SharedString::from(
            host.chars()
                .next()
                .unwrap_or('?')
                .to_uppercase()
                .to_string(),
        ))
        .into_any_element()
}
