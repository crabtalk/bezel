use crate::{
    DataStore,
    page::{
        Edit, Page, Present, Report, SnapshotDone,
        platform::{
            presentation::{Configuration, Presenter, Surface},
            runtime::Session,
        },
        script,
    },
    view::WebView,
};
use anyhow::{Context as _, Result, ensure};
use gpui::{
    self, App, Bounds, ElementInputHandler, Hitbox, Pixels, RenderImage, WeakEntity, Window,
    native_view::{NativeViewState, Viewport},
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{
    cell::{Cell, OnceCell, RefCell},
    collections::HashMap,
    io,
    rc::Rc,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU32, AtomicU64, AtomicUsize, Ordering},
    },
};

mod input;
mod presentation;
mod runtime;
mod transport;

pub(super) const BINDS_EDITS: bool = true;
type Evaluation = Box<dyn FnOnce(String) + Send>;

pub(crate) struct ConsoleSlot(Arc<AtomicUsize>);
impl Drop for ConsoleSlot {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::AcqRel);
    }
}

pub(crate) struct Message {
    pub(crate) epoch: u64,
    pub(crate) event: Event,
}

pub(crate) enum Event {
    Ready(Session),
    Changed,
    Error(String),
    Menu(Value),
    Clipboard(String),
    Evaluated(Box<Report>),
    Console {
        message: crate::ConsoleMessage,
        _slot: ConsoleSlot,
    },
}

#[derive(Clone, Default, Deserialize)]
pub(crate) struct Snapshot {
    pub(crate) url: Option<String>,
    pub(crate) title: String,
    pub(crate) loading: bool,
    pub(crate) can_back: bool,
    pub(crate) can_forward: bool,
    pub(crate) error: Option<String>,
}

pub(crate) struct Route {
    page: bool,
    pub(crate) epoch: AtomicU64,
    gate: Mutex<()>,
    reports: Option<async_channel::Sender<Report>>,
    surface: Mutex<Surface>,
    pub(crate) snapshot: Mutex<Snapshot>,
    pub(crate) input: Mutex<Value>,
    configuration: Mutex<Option<Configuration>>,
    dirty: AtomicBool,
    failure_reported: AtomicBool,
    console_slots: Arc<AtomicUsize>,
    evaluations: Mutex<HashMap<u32, Evaluation>>,
    next_evaluation: AtomicU32,
    snapshot_reply: Mutex<Option<(u32, SnapshotDone)>>,
    next_snapshot: AtomicU32,
    store_reply: Option<std::sync::mpsc::SyncSender<Value>>,
}

impl Route {
    fn new(reports: Option<async_channel::Sender<Report>>, page: bool) -> Self {
        Self {
            page,
            epoch: AtomicU64::new(1),
            gate: Mutex::new(()),
            reports,
            surface: Mutex::default(),
            snapshot: Mutex::default(),
            input: Mutex::new(json!({})),
            configuration: Mutex::new(None),
            dirty: AtomicBool::new(false),
            failure_reported: AtomicBool::new(false),
            console_slots: Arc::new(AtomicUsize::new(0)),
            evaluations: Mutex::default(),
            next_evaluation: AtomicU32::new(1),
            snapshot_reply: Mutex::default(),
            next_snapshot: AtomicU32::new(1),
            store_reply: None,
        }
    }

    fn store(reply: std::sync::mpsc::SyncSender<Value>) -> Self {
        let mut route = Self::new(None, false);
        route.store_reply = Some(reply);
        route
    }

    fn emit(&self, event: Event) {
        self.emit_at(self.epoch.load(Ordering::Acquire), event);
    }

    fn emit_at(&self, epoch: u64, event: Event) {
        if let Some(reports) = &self.reports {
            let _ = reports.try_send(Report::Linux(Message { epoch, event }));
        }
    }

    fn wake(&self) {
        if !self.dirty.swap(true, Ordering::AcqRel) {
            self.emit(Event::Changed);
        }
    }

    fn fail(&self, error: String) {
        if let Some((_, done)) = self.snapshot_reply.lock().unwrap().take() {
            done(Err(error.clone()));
        }
        if !self.failure_reported.swap(true, Ordering::AcqRel) {
            self.emit(Event::Error(error));
        }
        let evaluations = std::mem::take(&mut *self.evaluations.lock().unwrap());
        for (_, evaluation) in evaluations {
            evaluation("null".into());
        }
    }

    fn snapshot_result(&self, request: Option<u64>, result: Result<Arc<RenderImage>, String>) {
        let callback = {
            let mut pending = self.snapshot_reply.lock().unwrap();
            if pending
                .as_ref()
                .is_some_and(|(id, _)| Some(*id as u64) == request)
            {
                pending.take()
            } else {
                None
            }
        };
        if let Some((_, done)) = callback {
            done(result);
        }
    }

    fn packet(&self, kind: u8, bytes: &[u8]) -> io::Result<()> {
        let value = || serde_json::from_slice::<Value>(bytes).map_err(io::Error::other);
        match kind {
            b'E' => self.fail(String::from_utf8_lossy(bytes).into_owned()),
            b'S' => {
                *self.snapshot.lock().unwrap() =
                    serde_json::from_slice(bytes).map_err(io::Error::other)?;
                self.wake();
            }
            b'I' => {
                let value = value()?;
                let update = value
                    .as_object()
                    .ok_or_else(|| io::Error::other("invalid WPE input state"))?;
                let mut input = self.input.lock().unwrap();
                for (key, value) in update {
                    input[key] = value.clone();
                }
                drop(input);
                self.wake();
            }
            b'L' => {
                if let Some(report) =
                    Report::evaluated(std::str::from_utf8(bytes).map_err(io::Error::other)?)
                {
                    self.emit(Event::Evaluated(Box::new(report)));
                    return Ok(());
                }
                let source = self
                    .snapshot
                    .lock()
                    .unwrap()
                    .url
                    .clone()
                    .unwrap_or_default();
                if let Some(message) = crate::page::logged(
                    std::str::from_utf8(bytes).map_err(io::Error::other)?,
                    source,
                ) {
                    if self
                        .console_slots
                        .fetch_update(Ordering::AcqRel, Ordering::Acquire, |count| {
                            (count < 128).then_some(count + 1)
                        })
                        .is_ok()
                    {
                        self.emit(Event::Console {
                            message,
                            _slot: ConsoleSlot(self.console_slots.clone()),
                        });
                    }
                }
            }
            b'M' => self.emit(Event::Menu(value()?)),
            b'C' => self.emit(Event::Clipboard(
                String::from_utf8_lossy(bytes).into_owned(),
            )),
            b'N' => {
                if let Some(reports) = &self.reports {
                    let _ = reports
                        .try_send(Report::Opened(String::from_utf8_lossy(bytes).into_owned()));
                }
            }
            b'J' => {
                let value = value()?;
                if let Some(request) = value["request"]
                    .as_u64()
                    .and_then(|id| u32::try_from(id).ok())
                {
                    let callback = self.evaluations.lock().unwrap().remove(&request);
                    if let Some(callback) = callback {
                        callback(value["value"].as_str().unwrap_or("null").into());
                    }
                }
            }
            b'Q' => {
                let request = value()?["request"].as_u64();
                self.snapshot_result(request, Err("page frame is no longer available".into()));
            }
            b'D' => {
                if let Some(reply) = &self.store_reply {
                    let _ = reply.try_send(value()?);
                }
            }
            _ => return Err(io::Error::other("unknown WPE packet")),
        }
        Ok(())
    }
}

pub(crate) struct State {
    pub(crate) route: Arc<Route>,
    session: RefCell<Option<Session>>,
    presenter: RefCell<Option<Presenter>>,
    image: RefCell<Option<Arc<RenderImage>>>,
    pub(crate) view: OnceCell<WeakEntity<WebView>>,
    starting: Cell<bool>,
    failed: Cell<bool>,
    needs_load: Cell<bool>,
    focused: Cell<bool>,
    pub(crate) pressed: Cell<u8>,
    pub(crate) hovered: Cell<bool>,
    pub(crate) capture: Cell<Option<gpui::HitboxId>>,
    pub(crate) preedit: RefCell<String>,
}

impl State {
    pub(super) fn new(
        _cx: &App,
        _input: Rc<NativeViewState>,
        reports: async_channel::Sender<Report>,
    ) -> Self {
        Self {
            route: Arc::new(Route::new(Some(reports), true)),
            session: RefCell::new(None),
            presenter: RefCell::new(None),
            image: RefCell::new(None),
            view: OnceCell::new(),
            starting: Cell::new(false),
            failed: Cell::new(false),
            needs_load: Cell::new(true),
            focused: Cell::new(false),
            pressed: Cell::new(0),
            hovered: Cell::new(false),
            capture: Cell::new(None),
            preedit: RefCell::new(String::new()),
        }
    }

    pub(crate) fn ready(&self, session: Session) {
        self.presenter.replace(Some(Presenter::new(
            session.worker.outbox.clone(),
            session.id,
        )));
        self.session.replace(Some(session));
        self.starting.set(false);
    }

    pub(crate) fn failed(&self) {
        self.failed.set(true);
        self.focused.set(false);
        self.pressed.set(0);
    }

    pub(crate) fn snapshot(&self) -> Snapshot {
        self.route.dirty.store(false, Ordering::Release);
        self.route.snapshot.lock().unwrap().clone()
    }

    pub(crate) fn command(&self, command: Value) -> bool {
        self.session.borrow().as_ref().is_some_and(|session| {
            match session.worker.outbox.send(session.id, command) {
                Ok(()) => true,
                Err(error) => {
                    self.route.fail(error);
                    false
                }
            }
        })
    }

    fn start(&self, page: &Rc<Page>, window: &Window, cx: &App) {
        if self.starting.replace(true) {
            return;
        }
        let route = self.route.clone();
        let epoch = route.epoch.load(Ordering::Acquire);
        let store = page.store.borrow().clone();
        let user_agent = page.user_agent.borrow().clone();
        let script = if page.console.get() {
            format!("{}\n{}", script::MOVED, script::CONSOLE)
        } else {
            script::MOVED.into()
        };
        let gpu = window.sprite_atlas.dmabuf_target();
        cx.background_executor().spawn(async move {
            let result = (|| -> Result<Session> {
                let worker = runtime::Worker::for_store(&store, gpu)?;
                let session = worker.attach(&route, epoch)?;
                worker.outbox.send(session.id, json!({"cmd":"create", "userAgent":user_agent.unwrap_or_default(), "script":script})).map_err(anyhow::Error::msg)?;
                Ok(session)
            })();
            match result { Ok(session) => route.emit_at(epoch, Event::Ready(session)), Err(error) => route.emit_at(epoch, Event::Error(format!("{error:#}"))) }
        }).detach();
    }

    fn configure(&self, viewport: Viewport, scale: f32) -> Result<()> {
        ensure!(
            (0.5..=4.).contains(&scale),
            "WPE requires a device scale between 0.5 and 4"
        );
        ensure!(
            f32::from(viewport.width).ceil() * scale <= 8192.
                && f32::from(viewport.height).ceil() * scale <= 8192.,
            "WPE page dimensions exceed 8192 device pixels"
        );
        let mut current = self.route.configuration.lock().unwrap();
        if current
            .as_ref()
            .is_some_and(|current| current.viewport == viewport && current.scale == scale)
        {
            return Ok(());
        }
        let generation = current
            .map_or(Some(1), |current| current.generation.checked_add(1))
            .context("WPE viewport generations exhausted")?;
        let configuration = Configuration {
            viewport,
            scale,
            generation,
        };
        *current = Some(configuration);
        drop(current);
        self.command(configuration.command());
        Ok(())
    }

    fn reset(&self) {
        let _gate = self.route.gate.lock().unwrap();
        self.route.epoch.fetch_add(1, Ordering::AcqRel);
        self.route.dirty.store(false, Ordering::Release);
        self.route.failure_reported.store(false, Ordering::Release);
        *self.route.snapshot.lock().unwrap() = Snapshot::default();
        self.presenter.take();
        self.session.take();
        self.image.take();
        self.starting.set(false);
        self.failed.set(false);
        self.needs_load.set(true);
        self.route.surface.lock().unwrap().clear();
        self.route.configuration.lock().unwrap().take();
        *self.route.input.lock().unwrap() = json!({});
        self.preedit.borrow_mut().clear();
        self.pressed.set(0);
        self.hovered.set(false);
        self.capture.set(None);
        let evaluations = std::mem::take(&mut *self.route.evaluations.lock().unwrap());
        let snapshot = self.route.snapshot_reply.lock().unwrap().take();
        drop(_gate);
        if let Some((_, done)) = snapshot {
            done(Err("page restarted during snapshot".into()));
        }
        for (_, evaluation) in evaluations {
            evaluation("null".into());
        }
    }

    pub(crate) fn sync_focus(&self, focused: bool, active: bool) {
        self.focused.set(focused);
        if let Some(presenter) = self.presenter.borrow_mut().as_mut() {
            presenter.sync_focus(focused, active);
        }
        if !focused {
            self.pressed.set(0);
        }
    }
}

impl Present for State {
    const COMPOSITED: bool = true;
    fn available(&self, _window: &Window) -> bool {
        true
    }

    fn paint(
        &self,
        page: &Rc<Page>,
        bounds: Bounds<Pixels>,
        hitbox: &Hitbox,
        owner: u64,
        window: &mut Window,
        cx: &mut App,
    ) {
        if bounds.intersect(&window.content_mask().bounds).is_empty() {
            page.park();
            return;
        }
        let shown = page.placed.replace(Some(bounds)).is_some();
        page.window.set(Some(window.window_handle()));
        page.owner.set(owner);
        if self.failed.get() {
            return;
        }
        let viewport = page
            .presentation
            .get()
            .map(|presentation| presentation.viewport)
            .map_or_else(|| Viewport::try_from(bounds.size), Ok);
        let result = viewport.and_then(|viewport| self.configure(viewport, window.scale_factor()));
        if let Err(error) = result {
            self.failed.set(true);
            self.route.fail(format!("{error:#}"));
            return;
        }
        if self.session.borrow().is_none() {
            self.start(page, window, cx);
            return;
        }
        if !shown {
            self.command(json!({"cmd":"visible","value":1}));
        }
        if self.needs_load.replace(false) {
            if let Some(configuration) = *self.route.configuration.lock().unwrap() {
                self.command(configuration.command());
            }
            self.command(json!({"cmd":"load","url":page.url.borrow().as_str()}));
        }
        let focused = page.input.enabled.get() && page.focus.is_focused(window);
        self.sync_focus(focused, window.is_window_active());
        page.sync_pointer_capture(hitbox, window);

        if let Some(presenter) = self.presenter.borrow_mut().as_mut() {
            let configuration = *self.route.configuration.lock().unwrap();
            if let Err(error) = presenter.paint(
                &self.route.surface,
                configuration,
                &mut self.image.borrow_mut(),
                bounds,
                window,
            ) {
                self.route
                    .fail(format!("Browser presentation failed: {error:#}"));
            }
        }
        if page.input.enabled.get() {
            Page::install_input(page, hitbox, window);
            if let Some(cursor) = self.route.surface.lock().unwrap().cursor.as_deref() {
                window.set_cursor_style(input::cursor_style(cursor), hitbox);
            }
            if let Some(view) = self.view.get().and_then(WeakEntity::upgrade) {
                window.handle_input(&page.focus, ElementInputHandler::new(bounds, view), cx);
            }
        }
    }
}

impl Page {
    pub(crate) fn ready(&self) -> bool {
        self.platform.session.borrow().is_some() && !self.platform.needs_load.get()
    }

    pub(super) fn place(
        this: &Rc<Self>,
        bounds: Bounds<Pixels>,
        window: &mut Window,
        cx: &mut App,
    ) {
        let _ = (this, bounds, window, cx);
        unreachable!("composited pages use scene paint");
    }
    pub(crate) fn snapshot(&self, done: SnapshotDone) {
        let presenter = self.platform.presenter.borrow();
        let configuration = *self.platform.route.configuration.lock().unwrap();
        let Some((buffer, generation)) = presenter
            .as_ref()
            .and_then(Presenter::snapshot_frame)
            .filter(|(_, generation)| {
                configuration.is_some_and(|config| config.generation == *generation)
            })
        else {
            done(Err("page has no completed frame".into()));
            return;
        };
        let route = &self.platform.route;
        let Ok(request) =
            route
                .next_snapshot
                .fetch_update(Ordering::AcqRel, Ordering::Acquire, |id| id.checked_add(1))
        else {
            done(Err("snapshot request ids exhausted".into()));
            return;
        };
        let previous = route
            .snapshot_reply
            .lock()
            .unwrap()
            .replace((request, done));
        if let Some((_, done)) = previous {
            done(Err("snapshot superseded".into()));
        }
        self.platform.command(
            json!({"cmd":"snapshot","buffer":buffer,"generation":generation,"request":request}),
        );
    }

    pub(super) fn capture(&self) -> bool {
        false
    }
    pub(super) fn park(&self) {
        if self.placed.take().is_some() {
            self.platform.sync_focus(false, false);
            self.platform.command(json!({"cmd":"visible","value":0}));
        }
    }
    pub(crate) fn start(_window: &Window, _cx: &mut App) {}
    pub(crate) fn watch_keys(&self, _window: &Window) {}
    pub(crate) fn load(&self, url: String) {
        *self.url.borrow_mut() = url.clone();
        if self.platform.failed.get() {
            self.platform.reset();
        }
        self.platform.command(json!({"cmd":"load","url":url}));
        self.platform.route.wake();
    }
    pub(crate) fn back(&self) {
        self.platform.command(json!({"cmd":"back"}));
    }
    pub(crate) fn forward(&self) {
        self.platform.command(json!({"cmd":"forward"}));
    }
    pub(crate) fn history(&self) -> (bool, bool) {
        let snapshot = self.platform.route.snapshot.lock().unwrap();
        (snapshot.can_back, snapshot.can_forward)
    }
    pub(crate) fn reload(&self) {
        if self.platform.failed.get() {
            self.platform.reset();
        } else {
            self.platform.command(json!({"cmd":"reload"}));
        }
        self.platform.route.wake();
    }
    pub(crate) fn reload_bypassing_cache(&self) {
        if self.platform.failed.get() {
            self.platform.reset();
        } else {
            self.platform.command(json!({"cmd":"reload-bypass"}));
        }
        self.platform.route.wake();
    }
    pub(crate) fn location(&self) -> Option<String> {
        self.platform.route.snapshot.lock().unwrap().url.clone()
    }
    #[cfg(feature = "inspector")]
    pub(crate) fn open_inspector(&self) {}
    pub(crate) fn eval(&self, script: &str, done: impl FnOnce(String) + Send + 'static) -> bool {
        let route = &self.platform.route;
        let Ok(request) =
            route
                .next_evaluation
                .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |id| id.checked_add(1))
        else {
            return false;
        };
        let mut pending = route.evaluations.lock().unwrap();
        if pending.len() >= 64 {
            return false;
        }
        pending.insert(request, Box::new(done));
        drop(pending);
        if !self
            .platform
            .command(json!({"cmd":"eval","request":request,"script":script}))
        {
            route.evaluations.lock().unwrap().remove(&request);
            return false;
        }
        true
    }
    pub(crate) fn holds_keys(&self) -> bool {
        self.input.enabled.get() && self.platform.focused.get()
    }
    pub(crate) fn take_keys(&self) {
        if self.input.enabled.get() {
            self.platform.sync_focus(true, true);
        }
    }
    pub(crate) fn give_keys(&self) {
        self.platform.sync_focus(false, false);
    }
    pub(crate) fn edit(&self, edit: Edit) {
        let command = match edit {
            Edit::Copy => "copy",
            Edit::Cut => "cut",
            Edit::Paste => "paste",
            Edit::SelectAll => "select-all",
            Edit::Undo => "undo",
            Edit::Redo => "redo",
        };
        self.platform.command(json!({"cmd":command}));
    }
}

pub(super) fn clear_store(store: &DataStore, done: impl FnOnce(bool) + Send + 'static) {
    let store = store.clone();
    std::thread::spawn(move || {
        done(
            runtime::store_request(&store, "store-clear")
                .is_ok_and(|value| value["ok"].as_bool() == Some(true)),
        )
    });
}

pub(super) fn store_usage(
    store: &DataStore,
    done: impl FnOnce(Option<crate::Usage>) + Send + 'static,
) {
    let store = store.clone();
    std::thread::spawn(move || {
        let usage = runtime::store_request(&store, "store-usage")
            .ok()
            .and_then(|value| {
                let sites = value["sites"]
                    .as_array()?
                    .iter()
                    .filter_map(|site| site.as_str().map(str::to_owned))
                    .collect();
                Some(crate::Usage { sites })
            });
        done(usage);
    });
}
