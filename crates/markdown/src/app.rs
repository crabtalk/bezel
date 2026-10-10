//! App-owned configuration.

use gpui::App;

/// Configuration carried by the application. Import as `use markdown::AppExt as _;`.
pub trait AppExt {
    /// Installs the renderer for custom fenced blocks.
    fn set_block_renderer(&mut self, renderer: crate::BlockRenderer);

    /// Sets match and current washes; the callback keeps the current match stronger.
    fn set_find_paint(&mut self, paint: crate::FindPaint);

    /// Installs syntax highlighting and the language names offered by pickers.
    fn set_highlighter(
        &mut self,
        highlighter: crate::Highlighter,
        languages: impl IntoIterator<Item = impl Into<gpui::SharedString>>,
    );

    /// Configures markdown layout.
    fn set_markdown_layout(&mut self, layout: crate::Layout);

    /// Sets how far a mermaid diagram zooms.
    #[cfg(feature = "mermaid")]
    fn set_mermaid_zoom(&mut self, zoom: crate::mermaid::Zoom);

    /// Handles link clicks; without a handler links use `App::open_url`.
    fn set_link_handler(&mut self, handler: crate::LinkHandler);

    /// Installs the mark dialect shared by document readers and editors.
    fn set_marks(&mut self, marks: crate::Marks);

    /// Installs custom mark paint without changing how marks are parsed.
    fn set_mark_paint(&mut self, paint: crate::marks::Painter);

    /// Sets the paint for reader highlights.
    fn set_highlight_paint(&mut self, paint: crate::HighlightPaint);

    /// Installs the provider for link cards, chips and embeds.
    fn set_link_preview(&mut self, preview: crate::LinkPreview);

    /// Installs the painter of cards for links the app owns.
    fn set_link_card(&mut self, card: crate::LinkCard);

    /// Installs the painter of hover cards for links the app owns.
    fn set_link_hover(&mut self, hover: crate::LinkHover);

    /// Resolves source styles against the current theme and refreshes windows.
    fn set_source_style(&mut self, style: impl Fn(&theme::Theme) -> crate::SourceStyle + 'static);

    /// Sets the metrics used by document renderers.
    fn set_typography(&mut self, typography: crate::Typography);

    /// Reads highlight languages.
    fn highlight_languages(&self) -> &[gpui::SharedString];

    /// Reads markdown layout.
    fn markdown_layout(&self) -> crate::Layout;

    /// Reads typography.
    fn typography(&self) -> crate::Typography;

    /// Reads marks.
    fn marks(&self) -> crate::Marks;

    /// Reads source style.
    fn source_style(&self) -> crate::SourceStyle;
}

impl AppExt for App {
    fn set_block_renderer(&mut self, renderer: crate::BlockRenderer) {
        crate::block::set_block_renderer(self, renderer)
    }

    fn set_find_paint(&mut self, paint: crate::FindPaint) {
        crate::find::set_find_paint(self, paint)
    }

    fn set_highlighter(
        &mut self,
        highlighter: crate::Highlighter,
        languages: impl IntoIterator<Item = impl Into<gpui::SharedString>>,
    ) {
        crate::highlight::set_highlighter(self, highlighter, languages)
    }

    fn set_markdown_layout(&mut self, layout: crate::Layout) {
        crate::layout::set_layout(self, layout)
    }

    #[cfg(feature = "mermaid")]
    fn set_mermaid_zoom(&mut self, zoom: crate::mermaid::Zoom) {
        crate::mermaid::set_zoom(self, zoom)
    }

    fn set_link_handler(&mut self, handler: crate::LinkHandler) {
        crate::link::set_link_handler(self, handler)
    }

    fn set_marks(&mut self, marks: crate::Marks) {
        crate::marks::set_marks(self, marks)
    }

    fn set_mark_paint(&mut self, paint: crate::marks::Painter) {
        crate::marks::set_mark_paint(self, paint)
    }

    fn set_highlight_paint(&mut self, paint: crate::HighlightPaint) {
        crate::marks::set_highlight_paint(self, paint)
    }

    fn set_link_preview(&mut self, preview: crate::LinkPreview) {
        crate::preview::set_link_preview(self, preview)
    }

    fn set_link_card(&mut self, card: crate::LinkCard) {
        crate::preview::set_link_card(self, card)
    }

    fn set_link_hover(&mut self, hover: crate::LinkHover) {
        crate::preview::set_link_hover(self, hover)
    }

    fn set_source_style(&mut self, style: impl Fn(&theme::Theme) -> crate::SourceStyle + 'static) {
        crate::source_style::set_source_style(self, style)
    }

    fn set_typography(&mut self, typography: crate::Typography) {
        crate::typography::set_typography(self, typography)
    }

    fn highlight_languages(&self) -> &[gpui::SharedString] {
        crate::highlight::languages(self)
    }

    fn markdown_layout(&self) -> crate::Layout {
        crate::layout::Layout::of(self)
    }

    fn typography(&self) -> crate::Typography {
        crate::typography::Typography::of(self)
    }

    fn marks(&self) -> crate::Marks {
        crate::marks::Marks::of(self)
    }

    fn source_style(&self) -> crate::SourceStyle {
        crate::source_style::SourceStyle::of(self)
    }
}
