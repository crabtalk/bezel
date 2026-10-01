//! App-owned configuration.

use gpui::App;

/// Configuration carried by the application. Import as `use canvas::AppExt as _;`.
pub trait AppExt {
    /// Installs node kinds for canvas views without their own registry.
    fn set_canvas_kinds(&mut self, kinds: crate::Kinds);
}

impl AppExt for App {
    fn set_canvas_kinds(&mut self, kinds: crate::Kinds) {
        crate::kind::set_kinds(self, kinds)
    }
}
