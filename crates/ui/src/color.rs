//! Color picking: a preset [`Swatch`] set the app configures, and the system
//! color panel for anything outside it.
//!
//! The set is read through [`crate::AppExt::color_swatches`] and replaced with
//! [`crate::AppExt::set_color_swatches`]; until then it is [`default_swatches`].
//!
//! The panel is macOS only: [`open_panel`] returns `false` everywhere else and
//! nothing is shown. It is one per process. The latest [`open_panel`] owns it:
//! its `on_change` hears every pick from then on, and the previous owner's
//! stops.
//!
//! ```ignore
//! let this = cx.entity().downgrade();
//! color::open_panel(self.color, true, cx, move |color, cx| {
//!     this.update(cx, |view, cx| { view.color = color; cx.notify(); }).ok();
//! });
//! ```

use std::rc::Rc;

use gpui::{App, Global, Hsla, SharedString};
use theme::{Appearance, Theme};

/// One preset color, with a value per appearance.
#[derive(Clone, Debug, PartialEq)]
pub struct Swatch {
    /// What a tooltip or an accessibility label calls it.
    pub name: SharedString,
    pub light: Hsla,
    pub dark: Hsla,
}

impl Swatch {
    pub fn new(
        name: impl Into<SharedString>,
        light: impl Into<Hsla>,
        dark: impl Into<Hsla>,
    ) -> Self {
        Self {
            name: name.into(),
            light: light.into(),
            dark: dark.into(),
        }
    }

    /// The same color under both appearances.
    pub fn fixed(name: impl Into<SharedString>, color: impl Into<Hsla>) -> Self {
        let color = color.into();
        Self::new(name, color, color)
    }

    /// The value for `theme`'s appearance.
    pub fn resolve(&self, theme: &Theme) -> Hsla {
        match theme.appearance {
            Appearance::Light => self.light,
            Appearance::Dark => self.dark,
        }
    }
}

/// Apple's macOS system colors, red through brown.
pub fn default_swatches() -> Rc<[Swatch]> {
    let swatch = |name: &'static str, light: u32, dark: u32| {
        Swatch::new(name, gpui::rgb(light), gpui::rgb(dark))
    };
    Rc::from([
        swatch("Red", 0xFF3B30, 0xFF453A),
        swatch("Orange", 0xFF9500, 0xFF9F0A),
        swatch("Yellow", 0xFFCC00, 0xFFD60A),
        swatch("Green", 0x28CD41, 0x32D74B),
        swatch("Mint", 0x00C7BE, 0x63E6E2),
        swatch("Teal", 0x59ADC4, 0x6AC4DC),
        swatch("Cyan", 0x55BEF0, 0x5AC8F5),
        swatch("Blue", 0x007AFF, 0x0A84FF),
        swatch("Indigo", 0x5856D6, 0x5E5CE6),
        swatch("Purple", 0xAF52DE, 0xBF5AF2),
        swatch("Pink", 0xFF2D55, 0xFF375F),
        swatch("Brown", 0xA2845E, 0xAC8E68),
    ])
}

struct Swatches(Rc<[Swatch]>);

impl Global for Swatches {}

pub(crate) fn swatches(cx: &App) -> Rc<[Swatch]> {
    cx.try_global::<Swatches>()
        .map_or_else(default_swatches, |set| set.0.clone())
}

pub(crate) fn set_swatches(set: impl Into<Rc<[Swatch]>>, cx: &mut App) {
    cx.set_global(Swatches(set.into()));
    cx.refresh_windows();
}

/// Show the panel at `color` and report each change to `on_change`, with an
/// opacity slider when `opacity` is set. Colors cross in sRGB.
///
/// `on_change` runs on a later tick than the pick, never inside the caller's
/// update.
pub fn open_panel(
    color: Hsla,
    opacity: bool,
    cx: &mut App,
    on_change: impl Fn(Hsla, &mut App) + 'static,
) -> bool {
    imp::open(color, opacity, cx, on_change)
}

#[cfg(target_os = "macos")]
mod imp {
    use std::ptr::NonNull;

    use block2::RcBlock;
    use gpui::{App, Global, Hsla, Rgba};
    use objc2::MainThreadMarker;
    use objc2::rc::Retained;
    use objc2::runtime::{NSObjectProtocol, ProtocolObject};
    use objc2_app_kit::{
        NSColor, NSColorPanel, NSColorPanelColorDidChangeNotification, NSColorSpace,
    };
    use objc2_foundation::{NSNotification, NSNotificationCenter, NSOperationQueue};

    /// The observer the current owner registered; dropping it unregisters.
    struct Owner(Retained<ProtocolObject<dyn NSObjectProtocol>>);

    impl Drop for Owner {
        fn drop(&mut self) {
            // SAFETY: the token came from this center's `addObserverForName`.
            unsafe { NSNotificationCenter::defaultCenter().removeObserver(self.0.as_ref()) };
        }
    }

    impl Global for Owner {}

    pub(super) fn open(
        color: Hsla,
        opacity: bool,
        cx: &mut App,
        on_change: impl Fn(Hsla, &mut App) + 'static,
    ) -> bool {
        let Some(mtm) = MainThreadMarker::new() else {
            return false;
        };
        // Unregister first: `setColor` posts the change notification, and the
        // previous owner must not hear the new owner's starting color.
        if cx.has_global::<Owner>() {
            cx.remove_global::<Owner>();
        }
        let panel = NSColorPanel::sharedColorPanel(mtm);
        panel.setShowsAlpha(opacity);
        panel.setContinuous(true);
        panel.setColor(&to_ns(color));

        let async_cx = cx.to_async();
        let on_change = std::rc::Rc::new(on_change);
        let source = panel.clone();
        let block = RcBlock::new(move |_: NonNull<NSNotification>| {
            let Some(color) = from_ns(&source.color()) else {
                return;
            };
            let on_change = on_change.clone();
            async_cx
                .spawn(async move |cx| cx.update(|cx| on_change(color, cx)))
                .detach();
        });
        // SAFETY: the main queue runs the block on the main thread, the one
        // thread it and everything it captures belong to.
        let token = unsafe {
            NSNotificationCenter::defaultCenter().addObserverForName_object_queue_usingBlock(
                Some(NSColorPanelColorDidChangeNotification),
                Some(&panel),
                Some(&NSOperationQueue::mainQueue()),
                &block,
            )
        };
        cx.set_global(Owner(token));
        panel.orderFront(None);
        true
    }

    fn to_ns(color: Hsla) -> Retained<NSColor> {
        let Rgba { r, g, b, a } = color.to_rgb();
        NSColor::colorWithSRGBRed_green_blue_alpha(r.into(), g.into(), b.into(), a.into())
    }

    /// `None` for a pattern color, which has no sRGB form.
    fn from_ns(color: &NSColor) -> Option<Hsla> {
        let color = color.colorUsingColorSpace(&NSColorSpace::sRGBColorSpace())?;
        let (mut r, mut g, mut b, mut a) = (0.0, 0.0, 0.0, 0.0);
        // SAFETY: four valid out-pointers, on an RGB-space color.
        unsafe { color.getRed_green_blue_alpha(&mut r, &mut g, &mut b, &mut a) };
        Some(
            Rgba {
                r: r as f32,
                g: g as f32,
                b: b as f32,
                a: a as f32,
            }
            .into(),
        )
    }
}

#[cfg(not(target_os = "macos"))]
mod imp {
    use gpui::{App, Hsla};

    pub(super) fn open(
        _color: Hsla,
        _opacity: bool,
        _cx: &mut App,
        _on_change: impl Fn(Hsla, &mut App) + 'static,
    ) -> bool {
        false
    }
}
