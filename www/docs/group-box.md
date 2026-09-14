---
title: Group box
description: The section card and the rows inside it — a bordered container that thins to a translucent tint over glass.
---

```rust
use ui::{icons, widgets::Scaffolding};

theme
    .group_box()
    .child(
        theme
            .card_row(true)
            .child(theme.row_icon(icons::glyph::Monitor))
            .child(theme.row_title("Appearance")),
    )
    .child(
        theme
            .card_row(false)
            .child(theme.row_icon(icons::glyph::Folder))
            .child(theme.row_title("Storage")),
    )
```

`first` suppresses the top hairline on the opening row — CSS writes that as `first:border-t-0`, and gpui has no sibling selectors.

## Page rhythm

```rust
theme.page_column()
    .child(theme.page_header("Devices", Some(3)))
    .child(theme.page_subtitle("Signed in on three machines."))
    .child(theme.field_label("Theme"))
```

## API

```rust
pub trait Scaffolding: ThemeExt {
    /// The plate. Fill is `Theme::card_glass_bg` — the opaque card tone,
    /// thinned to a tint over glass.
    fn group_box(&self) -> Div;

    /// One row, split from the one above by a hairline.
    fn card_row(&self, first: bool) -> Div;

    /// The leading glyph, sized to the text beside it.
    fn row_icon(&self, icon: impl Into<Icon>) -> Div;

    /// Clipped rather than ellipsised, which keeps it in gpui's measure cache.
    fn row_title(&self, title: impl Into<SharedString>) -> Div;

    /// The quiet second line, joined by dots.
    fn meta_line(&self, fragments: Vec<AnyElement>) -> Div;

    // The rhythm around the card.
    fn page_column(&self) -> Div;
    fn page_header(&self, title: impl Into<SharedString>, count: Option<usize>) -> Div;
    fn page_subtitle(&self, copy: impl Into<SharedString>) -> Div;
    fn field_label(&self, label: impl Into<SharedString>) -> Div;

    /// A preview frame carrying the selection ring. The preview must round
    /// itself to `OPTION_CARD_RADIUS`.
    fn option_card(
        &self,
        label: impl Into<SharedString>,
        selected: bool,
        preview: AnyElement,
    ) -> Div;

    // ...
}
```

Hover is caller-owned — gpui panics on a second hover, so the wash to chain is `Theme::element_hover`.
