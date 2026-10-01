//! App-owned configuration.

use gpui::App;

/// Configuration carried by the application. Import as `use theme::AppExt as _;`.
pub trait AppExt {
    /// Reads brand.
    fn brand(&self) -> crate::Brand;

    /// Installs a brand, rebuilds the palette and refreshes window backgrounds.
    fn set_brand(&mut self, brand: crate::Brand);

    /// Reads appearance mode.
    fn appearance_mode(&self) -> crate::appearance::AppearanceMode;

    /// Changes the preference after `appearance::init`; repaints if needed.
    fn set_appearance_mode(&mut self, mode: crate::appearance::AppearanceMode);

    /// Sets the palette builder. Install before `appearance::init`, or apply afterward.
    fn set_palette(&mut self, build: fn(crate::Appearance) -> crate::Theme);

    /// Sets the shared type ladder’s base size in points and refreshes windows.
    fn set_base_text_size(&mut self, points: f32);
}

impl AppExt for App {
    fn brand(&self) -> crate::Brand {
        crate::brand::brand(self)
    }

    fn set_brand(&mut self, brand: crate::Brand) {
        crate::brand::set_brand(brand, self)
    }

    fn appearance_mode(&self) -> crate::appearance::AppearanceMode {
        crate::appearance::mode(self)
    }

    fn set_appearance_mode(&mut self, mode: crate::appearance::AppearanceMode) {
        crate::appearance::set_mode(mode, self)
    }

    fn set_palette(&mut self, build: fn(crate::Appearance) -> crate::Theme) {
        crate::theme::install::set_palette(build, self)
    }

    fn set_base_text_size(&mut self, points: f32) {
        crate::theme::typography::set_base_text_size(points, self)
    }
}
