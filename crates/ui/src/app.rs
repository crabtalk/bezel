//! App-owned configuration.

use gpui::App;

/// Configuration carried by the application. Import as `use ui::AppExt as _;`.
pub trait AppExt {
    /// Reads caret blink.
    fn caret_blink(&self) -> bool;

    /// Enables blinking or holds carets solid, then refreshes windows.
    fn set_caret_blink(&mut self, blink: bool);

    /// Reads caret shape.
    fn caret_shape(&self) -> crate::input::CaretShape;

    /// Sets text caret shape and refreshes windows. Defaults to `Bar`.
    fn set_caret_shape(&mut self, shape: crate::input::CaretShape);

    /// Reads scrollbar visibility.
    fn scrollbar_visibility(&self) -> crate::scroll::Visibility;

    /// Sets default scrollbar visibility and refreshes windows.
    fn set_scrollbar_visibility(&mut self, value: crate::scroll::Visibility);

    /// Reads the preset color set pickers offer.
    fn color_swatches(&self) -> std::rc::Rc<[crate::color::Swatch]>;

    /// Replaces the preset color set and refreshes windows. Defaults to
    /// [`crate::color::default_swatches`].
    fn set_color_swatches(&mut self, set: impl Into<std::rc::Rc<[crate::color::Swatch]>>);
}

impl AppExt for App {
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
}
