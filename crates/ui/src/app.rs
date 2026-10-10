//! App-owned configuration.

use gpui::App;

/// Configuration carried by the application. Import as `use ui::AppExt as _;`.
pub trait AppExt {
    /// Reads the caption button style. Defaults to `Rectangular`.
    fn caption_style(&self) -> crate::titlebar::CaptionStyle;

    /// Sets the caption button style and refreshes windows.
    fn set_caption_style(&mut self, style: crate::titlebar::CaptionStyle);

    /// Reads caret blink.
    fn caret_blink(&self) -> bool;

    /// Enables blinking or holds carets solid, then refreshes windows.
    fn set_caret_blink(&mut self, blink: bool);

    /// Reads caret shape.
    fn caret_shape(&self) -> crate::input::CaretShape;

    /// Sets text caret shape and refreshes windows. Defaults to `Bar`.
    fn set_caret_shape(&mut self, shape: crate::input::CaretShape);

    /// Reads block caret height.
    fn caret_height(&self) -> crate::input::CaretHeight;

    /// Sets block caret height and refreshes windows. Defaults to `Line`.
    fn set_caret_height(&mut self, height: crate::input::CaretHeight);

    /// Reads what a block caret paints as in an inactive window.
    fn inactive_caret(&self) -> crate::input::InactiveCaret;

    /// Sets it and refreshes windows. Defaults to `Hollow`.
    fn set_inactive_caret(&mut self, inactive: crate::input::InactiveCaret);

    /// Reads scrollbar visibility.
    fn scrollbar_visibility(&self) -> crate::scroll::Visibility;

    /// Sets default scrollbar visibility and refreshes windows.
    fn set_scrollbar_visibility(&mut self, value: crate::scroll::Visibility);

    /// Reads the preset color set pickers offer.
    fn color_swatches(&self) -> std::rc::Rc<[crate::color::Swatch]>;

    /// Replaces the preset color set and refreshes windows. Defaults to
    /// [`crate::color::default_swatches`].
    fn set_color_swatches(&mut self, set: impl Into<std::rc::Rc<[crate::color::Swatch]>>);

    /// Reads the menu indent step.
    fn menu_indent(&self) -> crate::menu::MenuIndent;

    /// Sets how far each [`crate::menu::Item::indented`] level moves a row in,
    /// and refreshes windows. Defaults to 12px.
    fn set_menu_indent(&mut self, indent: crate::menu::MenuIndent);

    /// Reads the menu panel widths.
    fn menu_width(&self) -> crate::menu::MenuWidth;

    /// Sets how wide menu panels sit, and refreshes windows. Defaults to
    /// 180px at least, and exactly 280px for a panel with a described row.
    fn set_menu_width(&mut self, width: crate::menu::MenuWidth);
}

impl AppExt for App {
    fn caption_style(&self) -> crate::titlebar::CaptionStyle {
        crate::titlebar::caption_style(self)
    }

    fn set_caption_style(&mut self, style: crate::titlebar::CaptionStyle) {
        self.set_global(style);
        self.refresh_windows();
    }

    fn caret_blink(&self) -> bool {
        crate::input::caret_blink(self)
    }

    fn set_caret_blink(&mut self, blink: bool) {
        crate::input::set_caret_blink(blink, self)
    }

    fn caret_shape(&self) -> crate::input::CaretShape {
        crate::input::caret::caret_shape(self)
    }

    fn set_caret_shape(&mut self, shape: crate::input::CaretShape) {
        crate::input::caret::set_caret_shape(shape, self)
    }

    fn caret_height(&self) -> crate::input::CaretHeight {
        crate::input::caret::caret_height(self)
    }

    fn set_caret_height(&mut self, height: crate::input::CaretHeight) {
        crate::input::caret::set_caret_height(height, self)
    }

    fn inactive_caret(&self) -> crate::input::InactiveCaret {
        crate::input::caret::inactive_caret(self)
    }

    fn set_inactive_caret(&mut self, inactive: crate::input::InactiveCaret) {
        crate::input::caret::set_inactive_caret(inactive, self)
    }

    fn scrollbar_visibility(&self) -> crate::scroll::Visibility {
        crate::scroll::overlay::visibility(self)
    }

    fn set_scrollbar_visibility(&mut self, value: crate::scroll::Visibility) {
        crate::scroll::overlay::set_visibility(value, self)
    }

    fn color_swatches(&self) -> std::rc::Rc<[crate::color::Swatch]> {
        crate::color::swatches(self)
    }

    fn set_color_swatches(&mut self, set: impl Into<std::rc::Rc<[crate::color::Swatch]>>) {
        crate::color::set_swatches(set, self)
    }

    fn menu_indent(&self) -> crate::menu::MenuIndent {
        crate::menu::indent(self)
    }

    fn set_menu_indent(&mut self, indent: crate::menu::MenuIndent) {
        crate::menu::set_indent(indent, self)
    }

    fn menu_width(&self) -> crate::menu::MenuWidth {
        crate::menu::width(self)
    }

    fn set_menu_width(&mut self, width: crate::menu::MenuWidth) {
        crate::menu::set_width(width, self)
    }
}
