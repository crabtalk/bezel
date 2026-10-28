use super::{Edit, Report};
use gpui::{App, ForegroundExecutor, RenderImage, Window};
use std::{cell::Cell, rc::Rc, sync::Arc};
use wry::WebViewExtWindows;

mod accelerator;

pub(super) const BINDS_EDITS: bool = false;

pub(super) struct State {
    /// Whether the page holds keyboard focus, as WebView2 last reported.
    focused: Rc<Cell<bool>>,
    keys: Rc<accelerator::Keys>,
    /// Runs focus moves between gpui updates: moving Win32 focus sends the
    /// window's messages synchronously.
    executor: ForegroundExecutor,
}

impl State {
    pub(super) fn new(cx: &App) -> Self {
        Self {
            focused: Rc::new(Cell::new(false)),
            keys: Rc::new(accelerator::Keys::new(cx)),
            executor: cx.foreground_executor().clone(),
        }
    }

    /// WebView2 reports focus, and a press with it.
    pub(super) fn attach(&self, view: &wry::WebView, reports: &async_channel::Sender<Report>) {
        use webview2_com::FocusChangedEventHandler;

        let controller = view.controller();
        accelerator::attach(&controller, self.keys.clone(), reports.clone());
        let (got, lost) = (self.focused.clone(), self.focused.clone());
        let pressed = reports.clone();
        let mut token = 0;
        // SAFETY: called on the thread that owns the controller.
        unsafe {
            let _ = controller.add_GotFocus(
                &FocusChangedEventHandler::create(Box::new(move |_, _| {
                    got.set(true);
                    let _ = pressed.try_send(Report::Pressed);
                    Ok(())
                })),
                &mut token,
            );
            let _ = controller.add_LostFocus(
                &FocusChangedEventHandler::create(Box::new(move |_, _| {
                    lost.set(false);
                    Ok(())
                })),
                &mut token,
            );
        }
    }

    pub(super) fn parked(&self) {}

    /// Call while the view's focus handle is focused.
    pub(super) fn watch_keys(&self, window: &Window) {
        self.keys.watch(window);
    }

    pub(super) fn holds_keys(&self, _view: &wry::WebView) -> bool {
        self.focused.get()
    }

    /// Moves focus into the page, unless it is hidden by then.
    pub(super) fn take_keys(&self, view: &wry::WebView) {
        use webview2_com::Microsoft::Web::WebView2::Win32::COREWEBVIEW2_MOVE_FOCUS_REASON_PROGRAMMATIC;

        let controller = view.controller();
        self.executor
            .spawn(async move {
                let mut visible = windows::core::BOOL::default();
                // SAFETY: on the thread that owns the controller.
                unsafe {
                    if controller.IsVisible(&mut visible).is_ok() && visible.as_bool() {
                        let _ = controller.MoveFocus(COREWEBVIEW2_MOVE_FOCUS_REASON_PROGRAMMATIC);
                    }
                }
            })
            .detach();
    }

    /// Moves focus to the window the page is in now.
    pub(super) fn give_keys(&self, view: &wry::WebView) {
        use windows::Win32::{Foundation::HWND, UI::Input::KeyboardAndMouse::SetFocus};

        let controller = view.controller();
        self.executor
            .spawn(async move {
                let mut parent = HWND::default();
                // SAFETY: on the thread that owns the controller.
                unsafe {
                    if controller.ParentWindow(&mut parent).is_ok() && !parent.is_invalid() {
                        let _ = SetFocus(Some(parent));
                    }
                }
            })
            .detach();
    }
}

pub(super) fn start(_window: &Window, _cx: &mut App) {}

/// Store identifiers are macOS's.
pub(super) fn store<'a>(
    builder: wry::WebViewBuilder<'a>,
    _identifier: Option<[u8; 16]>,
) -> wry::WebViewBuilder<'a> {
    builder
}

/// Moves the page's window into `window`.
pub(super) fn reparent(view: &wry::WebView, window: &Window) -> bool {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};

    let Ok(handle) = HasWindowHandle::window_handle(window) else {
        return false;
    };
    let RawWindowHandle::Win32(handle) = handle.as_raw() else {
        return false;
    };
    view.reparent(handle.hwnd.get())
        .inspect_err(|error| tracing::warn!(%error, "webview: reparent"))
        .is_ok()
}

pub(super) fn edit(_edit: Edit) {}

pub(super) fn back(view: &wry::WebView) {
    // SAFETY: called on the thread that owns the webview.
    let _ = unsafe { view.webview().GoBack() };
}

pub(super) fn forward(view: &wry::WebView) {
    // SAFETY: called on the thread that owns the webview.
    let _ = unsafe { view.webview().GoForward() };
}

/// Through the DevTools protocol: WebView2 has no reload that skips the cache.
#[cfg(feature = "inspector")]
pub(super) fn open_inspector(view: &wry::WebView) {
    view.open_devtools();
}

pub(super) fn reload_bypassing_cache(view: &wry::WebView) {
    use webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2CallDevToolsProtocolMethodCompletedHandler;
    use windows::core::w;

    // SAFETY: called on the thread that owns the webview.
    let _ = unsafe {
        view.webview().CallDevToolsProtocolMethod(
            w!("Page.reload"),
            w!(r#"{"ignoreCache":true}"#),
            None::<&ICoreWebView2CallDevToolsProtocolMethodCompletedHandler>,
        )
    };
}

pub(super) fn closed(_view: &wry::WebView) -> bool {
    false
}

pub(super) fn default_user_agent() -> Option<String> {
    None
}

/// The page's visible area as a PNG, through WebView2's `CapturePreview`.
pub(super) fn capture(
    view: &wry::WebView,
    done: impl FnOnce(Option<Arc<RenderImage>>) + 'static,
) -> bool {
    use webview2_com::{
        CapturePreviewCompletedHandler,
        Microsoft::Web::WebView2::Win32::COREWEBVIEW2_CAPTURE_PREVIEW_IMAGE_FORMAT_PNG,
    };
    use windows::Win32::UI::Shell::SHCreateMemStream;

    // SAFETY: an empty stream, with no initial bytes.
    let Some(stream) = (unsafe { SHCreateMemStream(None) }) else {
        return false;
    };
    let written = stream.clone();
    let handler = CapturePreviewCompletedHandler::create(Box::new(move |result| {
        done(result.ok().and_then(|()| pixels(&written)));
        Ok(())
    }));
    // SAFETY: called on the thread that owns the webview.
    unsafe {
        view.webview()
            .CapturePreview(
                COREWEBVIEW2_CAPTURE_PREVIEW_IMAGE_FORMAT_PNG,
                &stream,
                &handler,
            )
            .is_ok()
    }
}

/// The PNG `stream` holds, as the BGRA pixels a `RenderImage` holds.
fn pixels(stream: &windows::Win32::System::Com::IStream) -> Option<Arc<RenderImage>> {
    use windows::Win32::System::Com::{STREAM_SEEK_END, STREAM_SEEK_SET};

    let mut size = 0u64;
    let mut read = 0u32;
    // SAFETY: `bytes` holds `size` bytes, the stream's length.
    let bytes = unsafe {
        stream.Seek(0, STREAM_SEEK_END, Some(&mut size)).ok()?;
        stream.Seek(0, STREAM_SEEK_SET, None).ok()?;
        let mut bytes = vec![0u8; usize::try_from(size).ok()?];
        stream
            .Read(
                bytes.as_mut_ptr().cast(),
                u32::try_from(size).ok()?,
                Some(&mut read),
            )
            .ok()
            .ok()?;
        bytes.truncate(read as usize);
        bytes
    };
    let mut buffer = image::load_from_memory_with_format(&bytes, image::ImageFormat::Png)
        .ok()?
        .into_rgba8();
    for pixel in buffer.pixels_mut() {
        pixel.0.swap(0, 2);
    }
    Some(Arc::new(RenderImage::new([image::Frame::new(buffer)])))
}

/// WebView2 reaches a store only through a built page.
// TODO: clear a store with no page built on Windows.
pub(super) fn clear_store(_store: &crate::DataStore, done: impl FnOnce(bool) + Send + 'static) {
    done(false);
}

// TODO: report a store's usage on Windows.
pub(super) fn store_usage(
    _store: &crate::DataStore,
    done: impl FnOnce(Option<crate::Usage>) + Send + 'static,
) {
    done(None);
}
