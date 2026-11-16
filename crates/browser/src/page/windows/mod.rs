use crate::{
    DataStore, LoadState,
    page::{Edit, Page, Present, Report, script},
};
use anyhow::{Context as _, Error, Result, ensure};
use gpui::{
    self, App, Bounds, ForegroundExecutor, Hitbox, Pixels, Window,
    native_view::{NativeViewState, Viewport},
};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use std::{
    cell::{Cell, RefCell},
    path::PathBuf,
    rc::Rc,
    time::Duration,
};
use webview2_com::{
    self as callbacks, CoTaskMemPWSTR,
    Microsoft::Web::WebView2::Win32::{
        self as webview, ICoreWebView2, ICoreWebView2CompositionController,
        ICoreWebView2Controller, ICoreWebView2Environment10, ICoreWebView2Settings2,
    },
};
use windows::{
    Graphics::SizeInt32,
    System::{DispatcherQueue, DispatcherQueueController},
    UI::Composition::{Compositor, ContainerVisual},
    Win32::{
        Foundation::{self, HWND, RECT},
        System::WinRT::{self as winrt, DispatcherQueueOptions},
        UI::Input::KeyboardAndMouse,
    },
    core::{BOOL, Error as WindowsError, Interface as _, PCWSTR, PWSTR},
};
use windows_numerics::{Vector2, Vector3};

mod accelerator;
mod capture;
mod input;

pub(super) const BINDS_EDITS: bool = false;
const STARTUP_TIMEOUT: Duration = Duration::from_secs(15);
struct Apartment;
impl Drop for Apartment {
    fn drop(&mut self) {
        unsafe {
            winrt::RoUninitialize();
        }
    }
}
thread_local! {
    static APARTMENT: RefCell<Option<Apartment>> = const { RefCell::new(None) };
    static DISPATCHER: RefCell<Option<DispatcherQueueController>> = const { RefCell::new(None) };
}

struct Hosted {
    controller: ICoreWebView2Controller,
    composition: ICoreWebView2CompositionController,
    web: ICoreWebView2,
    _compositor: Compositor,
    root: ContainerVisual,
    content: ContainerVisual,
    capture: Option<capture::Capture>,
    geometry: Option<(Viewport, f32)>,
    parent: HWND,
}
impl Drop for Hosted {
    fn drop(&mut self) {
        self.capture.take();
        if let Err(error) = unsafe { self.composition.SetRootVisualTarget(None) } {
            tracing::debug!(%error, "detach WebView2 visual");
        }
        if let Err(error) = unsafe { self.controller.Close() } {
            tracing::debug!(%error, "close WebView2 controller");
        }
    }
}

#[derive(Clone)]
pub(super) struct Events {
    sender: async_channel::Sender<Report>,
    current: std::sync::Arc<std::sync::atomic::AtomicU64>,
    epoch: u64,
}
impl Events {
    pub(super) fn try_send(
        &self,
        report: Report,
    ) -> Result<(), async_channel::TrySendError<Report>> {
        if self.current.load(std::sync::atomic::Ordering::Acquire) != self.epoch {
            return Ok(());
        }
        self.sender.try_send(Report::Windows {
            epoch: self.epoch,
            report: Box::new(report),
        })
    }
}

pub(crate) struct State {
    hosted: RefCell<Option<Hosted>>,
    starting: Cell<bool>,
    pub(crate) epoch: std::sync::Arc<std::sync::atomic::AtomicU64>,
    pub(crate) failed: Cell<bool>,
    focused: Rc<Cell<bool>>,
    pub(crate) pressed: Cell<u8>,
    keys: Rc<accelerator::Keys>,
    executor: ForegroundExecutor,
    background: gpui::BackgroundExecutor,
    pub(crate) hovered: Cell<bool>,
    pub(crate) capture: Cell<Option<gpui::HitboxId>>,
    reports: async_channel::Sender<Report>,
}

impl State {
    pub(super) fn new(
        cx: &App,
        _input: Rc<NativeViewState>,
        reports: async_channel::Sender<Report>,
    ) -> Self {
        Self {
            hosted: RefCell::new(None),
            starting: Cell::new(false),
            epoch: std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0)),
            failed: Cell::new(false),
            focused: Rc::new(Cell::new(false)),
            pressed: Cell::new(0),
            keys: Rc::new(accelerator::Keys::new(cx)),
            executor: cx.foreground_executor().clone(),
            background: cx.background_executor().clone(),
            hovered: Cell::new(false),
            capture: Cell::new(None),
            reports,
        }
    }

    fn events(&self) -> Events {
        Events {
            sender: self.reports.clone(),
            current: self.epoch.clone(),
            epoch: self.epoch.load(std::sync::atomic::Ordering::Acquire),
        }
    }

    fn fail(&self, error: impl std::fmt::Display) {
        self.failed.set(true);
        let _ = self.events().try_send(Report::Failed(error.to_string()));
    }

    fn request(&self, page: &Rc<Page>, parent: HWND) {
        if self.failed.get() || self.starting.replace(true) {
            return;
        }
        let page = page.clone();
        let epoch = self.epoch.load(std::sync::atomic::Ordering::Acquire);
        self.executor.spawn(async move {
            let build = Hosted::new(&page, parent);
            let timer = page.platform.background.timer(STARTUP_TIMEOUT);
            let result = futures::future::select(Box::pin(build), Box::pin(timer)).await;
            let result = match result { futures::future::Either::Left((result, _)) => result, futures::future::Either::Right(_) => Err(anyhow::anyhow!("WebView2 startup timed out")) };
            if page.platform.epoch.load(std::sync::atomic::Ordering::Acquire) != epoch { return; }
            page.platform.starting.set(false);
            match result {
                Ok(hosted) => { page.platform.hosted.replace(Some(hosted)); let _ = page.reports.try_send(Report::Built); }
                Err(error) => { page.platform.fail(format!("WebView2 unavailable: {error:#}. Install the Microsoft WebView2 Runtime, then reload.")); }
            }
        }).detach();
    }

    fn reset(&self) {
        self.epoch.fetch_add(1, std::sync::atomic::Ordering::AcqRel);
        self.failed.set(false);
        self.hosted.take();
        self.starting.set(false);
        self.focused.set(false);
        self.pressed.set(0);
    }
}

impl Hosted {
    async fn new(page: &Rc<Page>, parent: HWND) -> Result<Self> {
        APARTMENT.with(|apartment| -> Result<()> {
            if apartment.borrow().is_none() {
                unsafe {
                    winrt::RoInitialize(winrt::RO_INIT_SINGLETHREADED)?;
                }
                apartment.replace(Some(Apartment));
            }
            Ok(())
        })?;
        if DispatcherQueue::GetForCurrentThread().is_err() {
            DISPATCHER.with(|queue| -> Result<()> {
                if queue.borrow().is_none() {
                    let controller = unsafe {
                        winrt::CreateDispatcherQueueController(DispatcherQueueOptions {
                            dwSize: std::mem::size_of::<DispatcherQueueOptions>() as u32,
                            threadType: winrt::DQTYPE_THREAD_CURRENT,
                            apartmentType: winrt::DQTAT_COM_STA,
                        })?
                    };
                    queue.replace(Some(controller));
                }
                Ok(())
            })?;
        }
        let store = page.store.borrow().clone();
        let directory = store.directory.clone().unwrap_or_else(|| {
            std::env::var_os("LOCALAPPDATA")
                .map(PathBuf::from)
                .unwrap_or_else(std::env::temp_dir)
                .join("bezel/browser")
        });
        let directory: Vec<u16> = directory
            .as_os_str()
            .to_string_lossy()
            .encode_utf16()
            .chain(Some(0))
            .collect();
        let (ready, result) = async_channel::bounded(1);
        let handler = callbacks::CreateCoreWebView2EnvironmentCompletedHandler::create(Box::new(
            move |status, environment| {
                let _ = ready.try_send(status.and_then(|()| {
                    environment.ok_or_else(|| WindowsError::from(Foundation::E_POINTER))
                }));
                Ok(())
            },
        ));
        unsafe {
            webview::CreateCoreWebView2EnvironmentWithOptions(
                PCWSTR::null(),
                PCWSTR(directory.as_ptr()),
                None::<&webview::ICoreWebView2EnvironmentOptions>,
                &handler,
            )?;
        }
        let environment = result.recv().await??;
        let options_environment: ICoreWebView2Environment10 = environment.cast()?;
        let options = unsafe { options_environment.CreateCoreWebView2ControllerOptions()? };
        unsafe {
            options.SetIsInPrivateModeEnabled(store.incognito)?;
        }
        let (ready, result) = async_channel::bounded(1);
        let handler = callbacks::CreateCoreWebView2CompositionControllerCompletedHandler::create(
            Box::new(move |status, controller| {
                let _ = ready.try_send(status.and_then(|()| {
                    controller.ok_or_else(|| WindowsError::from(Foundation::E_POINTER))
                }));
                Ok(())
            }),
        );
        unsafe {
            options_environment
                .CreateCoreWebView2CompositionControllerWithOptions(parent, &options, &handler)?;
        }
        let composition = result.recv().await??;
        let controller: ICoreWebView2Controller = composition.cast()?;
        let web = unsafe { controller.CoreWebView2()? };
        let compositor = Compositor::new()?;
        let root = compositor.CreateContainerVisual()?;
        let content = compositor.CreateContainerVisual()?;
        root.Children()?.InsertAtTop(&content)?;
        unsafe {
            composition.SetRootVisualTarget(&content)?;
            let settings = web.Settings()?;
            settings.SetAreDefaultContextMenusEnabled(true)?;
            settings.SetIsZoomControlEnabled(false)?;
            #[cfg(feature = "inspector")]
            settings.SetAreDevToolsEnabled(page.inspector.get())?;
            if let Some(agent) = page.user_agent.borrow().as_ref() {
                let agent: Vec<u16> = agent.encode_utf16().chain(Some(0)).collect();
                settings
                    .cast::<ICoreWebView2Settings2>()?
                    .SetUserAgent(PCWSTR(agent.as_ptr()))?;
            }
        }
        let hosted = Self {
            controller,
            composition,
            web,
            _compositor: compositor,
            root,
            content,
            capture: None,
            geometry: None,
            parent,
        };
        hosted.attach(page)?;
        let script = if page.console.get() {
            format!("{}\n{}", script::MOVED, script::CONSOLE)
        } else {
            script::MOVED.into()
        };
        let (ready, result) = async_channel::bounded(1);
        let handler = callbacks::AddScriptToExecuteOnDocumentCreatedCompletedHandler::create(
            Box::new(move |status, _| {
                let _ = ready.try_send(status);
                Ok(())
            }),
        );
        let script: Vec<u16> = script.encode_utf16().chain(Some(0)).collect();
        unsafe {
            hosted
                .web
                .AddScriptToExecuteOnDocumentCreated(PCWSTR(script.as_ptr()), &handler)?;
        }
        result.recv().await??;
        let url: Vec<u16> = page.url.borrow().encode_utf16().chain(Some(0)).collect();
        unsafe {
            hosted.web.Navigate(PCWSTR(url.as_ptr()))?;
        }
        Ok(hosted)
    }

    fn attach(&self, page: &Page) -> Result<()> {
        accelerator::attach(
            &self.controller,
            page.platform.keys.clone(),
            page.platform.events(),
        );
        let mut token = 0;
        unsafe {
            let reports = page.platform.events();
            self.composition.add_CursorChanged(
                &callbacks::CursorChangedEventHandler::create(Box::new(move |_, _| {
                    let _ = reports.try_send(Report::Frame);
                    Ok(())
                })),
                &mut token,
            )?;

            let reports = page.platform.events();
            let focus = page.platform.focused.clone();
            self.controller.add_GotFocus(
                &callbacks::FocusChangedEventHandler::create(Box::new(move |_, _| {
                    focus.set(true);
                    let _ = reports.try_send(Report::Pressed);
                    Ok(())
                })),
                &mut token,
            )?;
            let focus = page.platform.focused.clone();
            self.controller.add_LostFocus(
                &callbacks::FocusChangedEventHandler::create(Box::new(move |_, _| {
                    focus.set(false);
                    Ok(())
                })),
                &mut token,
            )?;
            let reports = page.platform.events();
            self.web.add_NavigationStarting(
                &callbacks::NavigationStartingEventHandler::create(Box::new(move |_, args| {
                    if let Some(args) = args {
                        let mut uri = PWSTR::null();
                        args.Uri(&mut uri)?;
                        let uri = CoTaskMemPWSTR::from(uri);
                        let _ = reports.try_send(Report::Load(LoadState::Started, uri.to_string()));
                    }
                    Ok(())
                })),
                &mut token,
            )?;
            let reports = page.platform.events();
            self.web.add_NavigationCompleted(
                &callbacks::NavigationCompletedEventHandler::create(Box::new(move |web, args| {
                    if let Some(web) = web {
                        let mut uri = PWSTR::null();
                        web.Source(&mut uri)?;
                        let uri = CoTaskMemPWSTR::from(uri);
                        let _ =
                            reports.try_send(Report::Load(LoadState::Finished, uri.to_string()));
                    }
                    if let Some(args) = args {
                        let mut success = BOOL::default();
                        args.IsSuccess(&mut success)?;
                        if !success.as_bool() {
                            let _ = reports.try_send(Report::Failed(
                                "WebView2 could not load this page. Reload to retry.".into(),
                            ));
                        }
                    }
                    Ok(())
                })),
                &mut token,
            )?;
            let reports = page.platform.events();
            self.web.add_DocumentTitleChanged(
                &callbacks::DocumentTitleChangedEventHandler::create(Box::new(move |web, _| {
                    if let Some(web) = web {
                        let mut title = PWSTR::null();
                        web.DocumentTitle(&mut title)?;
                        let title = CoTaskMemPWSTR::from(title);
                        let _ = reports.try_send(Report::Title(title.to_string()));
                    }
                    Ok(())
                })),
                &mut token,
            )?;
            let reports = page.platform.events();
            self.web.add_HistoryChanged(
                &callbacks::HistoryChangedEventHandler::create(Box::new(move |_, _| {
                    let _ = reports.try_send(Report::Moved);
                    Ok(())
                })),
                &mut token,
            )?;
            let reports = page.platform.events();
            self.web.add_WebMessageReceived(
                &callbacks::WebMessageReceivedEventHandler::create(Box::new(move |_, args| {
                    if let Some(args) = args {
                        let mut body = PWSTR::null();
                        args.TryGetWebMessageAsString(&mut body)?;
                        let body = CoTaskMemPWSTR::from(body);
                        let body = body.to_string();
                        let report = if body == "moved" {
                            Some(Report::Moved)
                        } else {
                            let mut source = PWSTR::null();
                            args.Source(&mut source)?;
                            let source = CoTaskMemPWSTR::from(source);
                            Report::evaluated(&body).or_else(|| {
                                crate::page::logged(&body, source.to_string()).map(Report::Console)
                            })
                        };
                        if let Some(report) = report {
                            let _ = reports.try_send(report);
                        }
                    }
                    Ok(())
                })),
                &mut token,
            )?;
            let reports = page.platform.events();
            self.web.add_NewWindowRequested(
                &callbacks::NewWindowRequestedEventHandler::create(Box::new(move |_, args| {
                    if let Some(args) = args {
                        let mut uri = PWSTR::null();
                        args.Uri(&mut uri)?;
                        let uri = CoTaskMemPWSTR::from(uri);
                        args.SetHandled(true)?;
                        let _ = reports.try_send(Report::Opened(uri.to_string()));
                    }
                    Ok(())
                })),
                &mut token,
            )?;
            let reports = page.platform.events();
            self.web.add_ProcessFailed(
                &callbacks::ProcessFailedEventHandler::create(Box::new(move |_, _| {
                    let _ = reports.try_send(Report::Failed(
                        "WebView2 process stopped. Reload to restart.".into(),
                    ));
                    Ok(())
                })),
                &mut token,
            )?;
        }
        Ok(())
    }

    fn configure(&mut self, page: &Page, bounds: Bounds<Pixels>, window: &Window) -> Result<()> {
        let scale = window.scale_factor();
        let viewport = page
            .presentation
            .get()
            .map(|p| p.viewport)
            .map_or_else(|| Viewport::try_from(bounds.size), Ok)?;
        let width = (f32::from(viewport.width) * scale).ceil() as i32;
        let height = (f32::from(viewport.height) * scale).ceil() as i32;
        ensure!(
            (1..=8192).contains(&width) && (1..=8192).contains(&height),
            "WebView2 viewport exceeds 8192 device pixels"
        );
        let parent = Page::parent(window)?;
        if parent != self.parent {
            unsafe {
                self.controller.SetParentWindow(parent)?;
            }
            self.parent = parent;
        }
        let left = (f32::from(bounds.origin.x) * scale).round() as i32;
        let top = (f32::from(bounds.origin.y) * scale).round() as i32;
        unsafe {
            self.controller.SetBounds(RECT {
                left,
                top,
                right: left + width,
                bottom: top + height,
            })?;
            if let Ok(controller) = self.controller.cast::<webview::ICoreWebView2Controller3>() {
                controller.SetShouldDetectMonitorScaleChanges(false)?;
                controller.SetRasterizationScale(f64::from(scale))?;
            }
            self.controller.NotifyParentWindowPositionChanged()?;
            self.controller.SetIsVisible(true)?;
        }
        self.content.SetSize(Vector2 {
            X: width as f32,
            Y: height as f32,
        })?;
        self.root.SetOffset(Vector3 {
            X: left as f32,
            Y: top as f32,
            Z: 0.,
        })?;
        self.root.SetScale(Vector3 {
            X: f32::from(bounds.size.width / viewport.width),
            Y: f32::from(bounds.size.height / viewport.height),
            Z: 1.,
        })?;
        if self.geometry != Some((viewport, scale)) {
            self.capture.take();
            let adapter = window
                .sprite_atlas
                .shared_texture_adapter()
                .context("GPUI renderer cannot accept WebView2 shared textures")?;
            self.capture = Some(capture::Capture::new(
                &self.content,
                SizeInt32 {
                    Width: width,
                    Height: height,
                },
                adapter,
                page.platform.events(),
            )?);
            self.geometry = Some((viewport, scale));
        }
        Ok(())
    }
}

impl Present for State {
    const COMPOSITED: bool = true;
    fn available(&self, window: &Window) -> bool {
        window.sprite_atlas.shared_texture_adapter().is_some()
    }
    fn paint(
        &self,
        page: &Rc<Page>,
        bounds: Bounds<Pixels>,
        hitbox: &Hitbox,
        owner: u64,
        window: &mut Window,
        _cx: &mut App,
    ) {
        if bounds.intersect(&window.content_mask().bounds).is_empty() {
            page.park();
            return;
        }
        page.window.set(Some(Window::window_handle(window)));
        page.placed.set(Some(bounds));
        page.owner.set(owner);
        if self.hosted.borrow().is_none() {
            match Page::parent(window) {
                Ok(parent) => self.request(page, parent),
                Err(error) => self.fail(error),
            };
            return;
        }
        let result = (|| -> Result<()> {
            let mut state = self.hosted.borrow_mut();
            let hosted = state.as_mut().context("WebView2 not ready")?;
            hosted.configure(page, bounds, window)?;
            if let Some(capture) = &mut hosted.capture {
                capture.paint(bounds, window)?;
            }
            Ok(())
        })();
        if let Err(error) = result {
            self.fail(format!("{error:#}"));
        }
        page.sync_pointer_capture(hitbox, window);
        self.keys.watch(window);
        if page.input.enabled.get() {
            Page::install_input(page, hitbox, window);
            window.set_cursor_style(self.cursor(), hitbox);
        }
    }
}

impl Page {
    fn parent(window: &Window) -> Result<HWND> {
        match HasWindowHandle::window_handle(window)
            .map_err(Error::msg)?
            .as_raw()
        {
            RawWindowHandle::Win32(handle) => Ok(HWND(handle.hwnd.get() as *mut _)),
            _ => anyhow::bail!("WebView2 requires a Windows host"),
        }
    }
    pub(crate) fn ready(&self) -> bool {
        self.platform.hosted.borrow().is_some()
    }
    pub(crate) fn start(_window: &Window, _cx: &mut App) {}
    pub(crate) fn watch_keys(&self, window: &Window) {
        self.platform.keys.watch(window);
    }
    pub(super) fn place(
        _this: &Rc<Self>,
        _bounds: Bounds<Pixels>,
        _window: &mut Window,
        _cx: &mut App,
    ) {
        unreachable!("composited pages use scene paint");
    }
    pub(crate) fn snapshot(&self, done: crate::page::SnapshotDone) {
        let hosted = self.platform.hosted.borrow();
        if let Some(capture) = hosted.as_ref().and_then(|hosted| hosted.capture.as_ref()) {
            capture.snapshot(done, &self.platform.background);
        } else {
            done(Err("page has no completed frame".into()));
        }
    }
    pub(super) fn capture(&self) -> bool {
        false
    }
    pub(super) fn park(&self) {
        self.placed.set(None);
        self.give_keys();
        if let Some(hosted) = self.platform.hosted.borrow_mut().as_mut() {
            hosted.capture.take();
            hosted.geometry = None;
            if let Err(error) = unsafe { hosted.controller.SetIsVisible(false) } {
                tracing::debug!(%error, "hide WebView2");
            }
        }
    }
    pub(crate) fn load(&self, url: String) {
        *self.url.borrow_mut() = url.clone();
        if self.platform.failed.get() {
            self.platform.reset();
            let _ = self.reports.try_send(Report::Built);
            return;
        }
        if let Some(hosted) = self.platform.hosted.borrow().as_ref() {
            let url: Vec<_> = url.encode_utf16().chain(Some(0)).collect();
            if let Err(error) = unsafe { hosted.web.Navigate(PCWSTR(url.as_ptr())) } {
                self.platform.fail(error);
            }
        }
    }
    pub(crate) fn reload(&self) {
        if self.platform.failed.get() {
            if let Some(url) = self.location() {
                *self.url.borrow_mut() = url;
            }
            self.platform.reset();
            let _ = self.reports.try_send(Report::Built);
        } else if let Some(hosted) = self.platform.hosted.borrow().as_ref() {
            if let Err(error) = unsafe { hosted.web.Reload() } {
                self.platform.fail(error);
            }
        }
    }
    pub(crate) fn reload_bypassing_cache(&self) {
        if self.platform.failed.get() {
            self.reload();
            return;
        }
        if let Some(hosted) = self.platform.hosted.borrow().as_ref() {
            if let Err(error) = unsafe {
                hosted.web.CallDevToolsProtocolMethod(
                    windows::core::w!("Page.reload"),
                    windows::core::w!(r#"{"ignoreCache":true}"#),
                    None::<&webview::ICoreWebView2CallDevToolsProtocolMethodCompletedHandler>,
                )
            } {
                self.platform.fail(error);
            }
        }
    }
    pub(crate) fn back(&self) {
        if let Some(hosted) = self.platform.hosted.borrow().as_ref() {
            if let Err(error) = unsafe { hosted.web.GoBack() } {
                self.platform.fail(error);
            }
        }
    }
    pub(crate) fn forward(&self) {
        if let Some(hosted) = self.platform.hosted.borrow().as_ref() {
            if let Err(error) = unsafe { hosted.web.GoForward() } {
                self.platform.fail(error);
            }
        }
    }
    pub(crate) fn history(&self) -> (bool, bool) {
        let hosted = self.platform.hosted.borrow();
        let Some(hosted) = hosted.as_ref() else {
            return (false, false);
        };
        let mut back = BOOL::default();
        let mut forward = BOOL::default();
        unsafe {
            let _ = hosted.web.CanGoBack(&mut back);
            let _ = hosted.web.CanGoForward(&mut forward);
        }
        (back.as_bool(), forward.as_bool())
    }
    pub(crate) fn location(&self) -> Option<String> {
        let hosted = self.platform.hosted.borrow();
        let hosted = hosted.as_ref()?;
        let mut uri = PWSTR::null();
        unsafe {
            hosted.web.Source(&mut uri).ok()?;
        }
        Some(CoTaskMemPWSTR::from(uri).to_string())
    }
    pub(crate) fn eval(&self, script: &str, done: impl Fn(String) + Send + 'static) -> bool {
        let hosted = self.platform.hosted.borrow();
        let Some(hosted) = hosted.as_ref() else {
            return false;
        };
        let script: Vec<_> = script.encode_utf16().chain(Some(0)).collect();
        let handler =
            callbacks::ExecuteScriptCompletedHandler::create(Box::new(move |status, result| {
                done(if status.is_ok() {
                    result
                } else {
                    "null".into()
                });
                Ok(())
            }));
        unsafe {
            hosted
                .web
                .ExecuteScript(PCWSTR(script.as_ptr()), &handler)
                .is_ok()
        }
    }
    pub(crate) fn holds_keys(&self) -> bool {
        self.platform.focused.get()
    }
    pub(crate) fn take_keys(&self) {
        if !self.input.enabled.get() {
            return;
        }
        let controller = self
            .platform
            .hosted
            .borrow()
            .as_ref()
            .map(|hosted| hosted.controller.clone());
        self.platform
            .executor
            .spawn(async move {
                if let Some(controller) = controller {
                    if let Err(error) = unsafe {
                        controller.MoveFocus(webview::COREWEBVIEW2_MOVE_FOCUS_REASON_PROGRAMMATIC)
                    } {
                        tracing::debug!(%error, "focus WebView2");
                    }
                }
            })
            .detach();
    }
    pub(crate) fn give_keys(&self) {
        if !self.platform.focused.replace(false) {
            return;
        }
        self.platform.pressed.set(0);
        let parent = self
            .platform
            .hosted
            .borrow()
            .as_ref()
            .map(|hosted| hosted.parent);
        self.platform
            .executor
            .spawn(async move {
                if let Some(parent) = parent {
                    if let Err(error) = unsafe { KeyboardAndMouse::SetFocus(Some(parent)) } {
                        tracing::debug!(%error, "return browser focus");
                    }
                }
            })
            .detach();
    }
    pub(crate) fn edit(&self, _edit: Edit) {}
    #[cfg(feature = "inspector")]
    pub(crate) fn open_inspector(&self) {
        if let Some(hosted) = self.platform.hosted.borrow().as_ref() {
            if let Err(error) = unsafe { hosted.web.OpenDevToolsWindow() } {
                self.platform.fail(error);
            }
        }
    }
}

pub(super) fn clear_store(_store: &DataStore, done: impl FnOnce(bool) + Send + 'static) {
    done(false);
}
pub(super) fn store_usage(
    _store: &DataStore,
    done: impl FnOnce(Option<crate::Usage>) + Send + 'static,
) {
    done(None);
}
