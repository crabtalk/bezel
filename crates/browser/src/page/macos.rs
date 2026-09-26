use super::{Edit, Report};
use gpui::{App, RenderImage, Window};
use std::{
    cell::{Cell, OnceCell},
    sync::Arc,
};
use wry::WebViewExtMacOS;

mod cursor;

pub(super) const BINDS_EDITS: bool = true;

pub(super) struct State {
    cursor: OnceCell<cursor::Watch>,
}

impl State {
    pub(super) fn new(_cx: &App) -> Self {
        Self {
            cursor: OnceCell::new(),
        }
    }

    pub(super) fn attach(&self, view: &wry::WebView, _reports: &async_channel::Sender<Report>) {
        let _ = self.cursor.set(cursor::Watch::new(&view.webview()));
    }

    pub(super) fn parked(&self) {
        if let Some(cursor) = self.cursor.get() {
            cursor.release();
        }
    }

    pub(super) fn watch_keys(&self, _window: &Window) {}

    /// Whether the page, or a view inside it, is its window's first responder.
    pub(super) fn holds_keys(&self, view: &wry::WebView) -> bool {
        use objc2_app_kit::NSView;

        let page = view.webview();
        let Some(window) = page.window() else {
            return false;
        };
        window
            .firstResponder()
            .and_then(|responder| responder.downcast::<NSView>().ok())
            .is_some_and(|responder| responder.isDescendantOf(&page))
    }
}

pub(super) fn start(_window: &Window, _cx: &mut App) {}

/// Run in every frame. Any script in the page can post the same message.
const PRESSED: &str = "addEventListener('mousedown', () => \
    window.webkit.messageHandlers.ipc.postMessage('pressed'), true);";

pub(super) fn build(
    builder: wry::WebViewBuilder<'_>,
    window: &Window,
) -> Option<wry::Result<wry::WebView>> {
    Some(
        builder
            .with_initialization_script_for_main_only(PRESSED, false)
            .build_as_child(window),
    )
}

/// Sent down the key window's responder chain, where the page's view is
/// first while it holds keys.
pub(super) fn edit(edit: Edit) {
    use objc2::{MainThreadMarker, sel};
    use objc2_app_kit::NSApplication;

    let Some(mtm) = MainThreadMarker::new() else {
        return;
    };
    let action = match edit {
        Edit::Copy => sel!(copy:),
        Edit::Cut => sel!(cut:),
        Edit::Paste => sel!(paste:),
        Edit::SelectAll => sel!(selectAll:),
        Edit::Undo => sel!(undo:),
        Edit::Redo => sel!(redo:),
    };
    // SAFETY: a nil target resolves along the responder chain, and every
    // action here takes a sender, which may be nil.
    unsafe { NSApplication::sharedApplication(mtm).sendAction_to_from(action, None, None) };
}

pub(super) fn back(view: &wry::WebView) {
    // SAFETY: called on the main thread.
    unsafe { view.webview().goBack() };
}

pub(super) fn forward(view: &wry::WebView) {
    // SAFETY: called on the main thread.
    unsafe { view.webview().goForward() };
}

/// Takes a still of the page's visible rect, at the backing scale. `done`
/// runs on the main thread.
pub(super) fn capture(
    view: &wry::WebView,
    done: impl FnOnce(Option<Arc<RenderImage>>) + 'static,
) -> bool {
    use block2::RcBlock;
    use objc2_app_kit::NSImage;
    use objc2_foundation::NSError;

    let done = Cell::new(Some(done));
    let block = RcBlock::new(move |picture: *mut NSImage, _: *mut NSError| {
        // SAFETY: WebKit hands a valid image, or null with an error.
        let still = unsafe { picture.as_ref() }.and_then(pixels);
        if let Some(done) = done.take() {
            done(still);
        }
    });
    // SAFETY: called on the main thread. No configuration takes the visible
    // rect.
    unsafe {
        view.webview()
            .takeSnapshotWithConfiguration_completionHandler(None, &block);
    }
    true
}

/// An `NSImage` is drawn into a bitmap to become pixels at all.
fn pixels(picture: &objc2_app_kit::NSImage) -> Option<Arc<RenderImage>> {
    use objc2_core_foundation::{CGPoint, CGRect, CGSize};
    use objc2_core_graphics::{
        CGBitmapContextCreate, CGColorSpace, CGContext, CGImage, CGImageAlphaInfo,
        CGImageByteOrderInfo, kCGColorSpaceSRGB,
    };

    // SAFETY: a null rect and no context ask for the best representation.
    let picture =
        unsafe { picture.CGImageForProposedRect_context_hints(std::ptr::null_mut(), None, None) }?;
    let (width, height) = (
        CGImage::width(Some(&picture)),
        CGImage::height(Some(&picture)),
    );
    if width == 0 || height == 0 {
        return None;
    }
    let mut bytes = vec![0u8; width * height * 4];
    // SAFETY: reads an immutable CoreGraphics constant.
    let space = CGColorSpace::with_name(Some(unsafe { kCGColorSpaceSRGB }))?;
    // BGRA in memory, which is what `RenderImage` holds.
    let info = CGImageAlphaInfo::PremultipliedFirst.0 | CGImageByteOrderInfo::Order32Little.0;
    // SAFETY: `bytes` holds `height` rows of `width * 4` bytes and outlives
    // the context.
    let context = unsafe {
        CGBitmapContextCreate(
            bytes.as_mut_ptr().cast(),
            width,
            height,
            8,
            width * 4,
            Some(&space),
            info,
        )
    }?;
    let rect = CGRect::new(CGPoint::ZERO, CGSize::new(width as f64, height as f64));
    CGContext::draw_image(Some(&context), rect, Some(&picture));
    drop(context);
    let buffer =
        image::RgbaImage::from_raw(width.try_into().ok()?, height.try_into().ok()?, bytes)?;
    Some(Arc::new(RenderImage::new([image::Frame::new(buffer)])))
}

/// Whether the page's window has closed. wry's `set_bounds` and `focus`
/// unwrap the window.
pub(super) fn closed(view: &wry::WebView) -> bool {
    view.webview().window().is_none()
}

/// A WKWebView reports `AppleWebKit/605.1.15 (KHTML, like Gecko)` and no
/// browser, and some sites (Google) serve an unknown browser a basic page.
pub(super) fn default_user_agent() -> Option<String> {
    use objc2_foundation::{NSBundle, NSString};

    let version = NSBundle::bundleWithPath(&NSString::from_str("/Applications/Safari.app"))
        .and_then(|safari| {
            safari.objectForInfoDictionaryKey(&NSString::from_str("CFBundleShortVersionString"))
        })
        .and_then(|version| version.downcast::<NSString>().ok())
        .map_or_else(|| "26.0".to_owned(), |version| version.to_string());
    Some(format!(
        "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 \
         (KHTML, like Gecko) Version/{version} Safari/605.1.15"
    ))
}
