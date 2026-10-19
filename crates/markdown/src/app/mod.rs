//! App-owned configuration.

pub mod layout;
pub mod link;
pub mod picture;
pub mod preview;
pub mod typography;

use gpui::App;

/// Configuration carried by the application. Import as `use markdown::AppExt as _;`.
pub trait AppExt {
    /// Installs the renderer for custom fenced blocks.
    fn set_block_renderer(&mut self, renderer: crate::BlockRenderer);

    /// Sets match and current washes; the callback keeps the current match stronger.
    fn set_find_paint(&mut self, paint: crate::FindPaint);

    /// Sets the washes behind comment ranges.
    fn set_comment_paint(&mut self, paint: crate::CommentPaint);

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
    fn set_mermaid_zoom(&mut self, zoom: crate::fence::mermaid::Zoom);

    /// Installs what the app does with links — see [`crate::Links`].
    fn set_links<L: crate::Links>(&mut self);

    /// Installs the mark dialect shared by document readers and editors.
    fn set_marks(&mut self, marks: crate::Marks);

    /// Installs custom mark paint without changing how marks are parsed.
    fn set_mark_paint(&mut self, paint: crate::paint::marks::Painter);

    /// Sets the paint for reader highlights.
    fn set_highlight_paint(&mut self, paint: crate::HighlightPaint);

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
        crate::fence::set_block_renderer(self, renderer)
    }

    fn set_find_paint(&mut self, paint: crate::FindPaint) {
        crate::paint::find::set_find_paint(self, paint)
    }

    fn set_comment_paint(&mut self, paint: crate::CommentPaint) {
        crate::paint::comment::set_comment_paint(self, paint)
    }

    fn set_highlighter(
        &mut self,
        highlighter: crate::Highlighter,
        languages: impl IntoIterator<Item = impl Into<gpui::SharedString>>,
    ) {
        crate::fence::highlight::set_highlighter(self, highlighter, languages)
    }

    fn set_markdown_layout(&mut self, layout: crate::Layout) {
        crate::app::layout::set_layout(self, layout)
    }

    #[cfg(feature = "mermaid")]
    fn set_mermaid_zoom(&mut self, zoom: crate::fence::mermaid::Zoom) {
        crate::fence::mermaid::set_zoom(self, zoom)
    }

    fn set_links<L: crate::Links>(&mut self) {
        crate::app::link::set_links::<L>(self)
    }

    fn set_marks(&mut self, marks: crate::Marks) {
        crate::paint::marks::set_marks(self, marks)
    }

    fn set_mark_paint(&mut self, paint: crate::paint::marks::Painter) {
        crate::paint::marks::set_mark_paint(self, paint)
    }

    fn set_highlight_paint(&mut self, paint: crate::HighlightPaint) {
        crate::paint::marks::set_highlight_paint(self, paint)
    }

    fn set_source_style(&mut self, style: impl Fn(&theme::Theme) -> crate::SourceStyle + 'static) {
        crate::source::style::set_source_style(self, style)
    }

    fn set_typography(&mut self, typography: crate::Typography) {
        crate::app::typography::set_typography(self, typography)
    }

    fn highlight_languages(&self) -> &[gpui::SharedString] {
        crate::fence::highlight::languages(self)
    }

    fn markdown_layout(&self) -> crate::Layout {
        crate::app::layout::Layout::of(self)
    }

    fn typography(&self) -> crate::Typography {
        crate::app::typography::Typography::of(self)
    }

    fn marks(&self) -> crate::Marks {
        crate::marks::Marks::of(self)
    }

    fn source_style(&self) -> crate::SourceStyle {
        crate::source::style::SourceStyle::of(self)
    }
}
