//! App-owned configuration.

use gpui::App;

/// Configuration carried by the application. Import as `use editor::AppExt as _;`.
pub trait AppExt {
    /// Reads editor layout.
    fn editor_layout(&self) -> crate::Layout;

    /// Reads editor text size.
    fn editor_text_size(&self) -> crate::TextSize;

    /// Configures editor layout.
    fn set_editor_layout(&mut self, layout: crate::Layout);

    /// Sets editor zoom steps and limits, not an individual editor’s base size.
    fn set_editor_text_size(&mut self, text_size: crate::TextSize);

    /// Installs the image store used when pasting pictures.
    fn set_image_store(&mut self, store: crate::ImageStore);

    /// Returns the installed image store, or the default that keeps nothing.
    fn image_store(&self) -> crate::ImageStore;

    /// Overrides clipboard paste; returning `None` keeps the default behavior.
    /// File drops are not passed to this handler.
    fn set_paste_handler(&mut self, handler: crate::PasteHandler);

    /// Replaces the slash menu's items, which are [`crate::slash_defaults`]
    /// until this is called.
    fn set_slash_items(&mut self, items: Vec<crate::SlashItem>);

    /// Installs what the `@` menu lists, in editors whose
    /// [`crate::Chrome::mention`] is on.
    fn set_mention_source(&mut self, source: crate::MentionSource);

    /// Adjusts every open document by the given points and refreshes windows.
    fn adjust_editor_text_size(&mut self, points: f32);

    /// Resets the shared adjustment to zero and refreshes windows.
    fn reset_editor_text_size(&mut self);

    /// Returns the shared adjustment from each document’s base size, in points.
    fn editor_text_size_adjustment(&self) -> f32;
}

impl AppExt for App {
    fn editor_layout(&self) -> crate::Layout {
        crate::layout::Layout::of(self)
    }

    fn editor_text_size(&self) -> crate::TextSize {
        crate::text_size::TextSize::of(self)
    }

    fn set_editor_layout(&mut self, layout: crate::Layout) {
        crate::layout::set_layout(self, layout)
    }

    fn set_editor_text_size(&mut self, text_size: crate::TextSize) {
        crate::text_size::set_text_size(self, text_size)
    }

    fn set_image_store(&mut self, store: crate::ImageStore) {
        crate::editor::image::set_image_store(self, store)
    }

    fn image_store(&self) -> crate::ImageStore {
        crate::editor::image::store(self)
    }

    fn set_paste_handler(&mut self, handler: crate::PasteHandler) {
        self.set_global(crate::paste::Installed(handler));
    }

    fn set_mention_source(&mut self, source: crate::MentionSource) {
        self.set_global(crate::mention::Installed(source));
    }

    fn set_slash_items(&mut self, items: Vec<crate::SlashItem>) {
        self.set_global(crate::slash::Installed(items));
    }

    fn adjust_editor_text_size(&mut self, points: f32) {
        crate::text_size::adjust_text_size(self, points)
    }

    fn reset_editor_text_size(&mut self) {
        crate::text_size::reset_text_size(self)
    }

    fn editor_text_size_adjustment(&self) -> f32 {
        crate::text_size::text_size_adjustment(self)
    }
}
