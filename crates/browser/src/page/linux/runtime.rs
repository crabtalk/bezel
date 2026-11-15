use crate::{
    DataStore,
    page::platform::{
        Route, presentation,
        transport::{self, Outbox, Reader},
    },
};
use anyhow::{Context as _, Result, bail, ensure};
use gpui::external_image::DmabufTarget;
use serde_json::{Value, json};
use sha2::{Digest as _, Sha256};
use std::{
    collections::HashMap,
    fs, io,
    net::Shutdown,
    os::{
        fd::OwnedFd,
        unix::{
            fs::{OpenOptionsExt as _, PermissionsExt as _},
            net::UnixStream,
        },
    },
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{
        Arc, Condvar, Mutex, OnceLock, Weak,
        atomic::{AtomicBool, AtomicU32, Ordering},
    },
    thread,
    time::{Duration, Instant},
};

const HELPER: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/bezel-wpe"));
const STARTUP_TIMEOUT: Duration = Duration::from_secs(10);
struct Registration {
    route: Weak<Route>,
    epoch: u64,
}

type Profiles = HashMap<PathBuf, (Weak<Worker>, Weak<Process>)>;
static EXTRACTION: AtomicU32 = AtomicU32::new(0);
static PROFILES: OnceLock<Mutex<Profiles>> = OnceLock::new();

pub(crate) struct Session {
    pub(super) worker: Arc<Worker>,
    pub(super) id: u32,
}

impl Drop for Session {
    fn drop(&mut self) {
        self.worker.routes.lock().unwrap().remove(&self.id);
        let _ = self.worker.outbox.send(self.id, json!({"cmd":"close"}));
    }
}

pub(crate) struct Worker {
    pub(super) outbox: Outbox,
    pub(super) stopped: Arc<AtomicBool>,
    routes: Arc<Mutex<HashMap<u32, Registration>>>,
    next_id: AtomicU32,
    process: Arc<Process>,
}

struct Process {
    child: Mutex<Option<Child>>,
    finished: Mutex<bool>,
    changed: Condvar,
}

impl Process {
    fn stop(self: &Arc<Self>) {
        let child = self.child.lock().unwrap().take();
        if let Some(mut child) = child {
            let process = self.clone();
            thread::Builder::new()
                .name("wpe-shutdown".into())
                .spawn(move || {
                    let deadline = Instant::now() + Duration::from_secs(3);
                    loop {
                        match child.try_wait() {
                            Ok(Some(_)) => break,
                            Ok(None) if Instant::now() < deadline => {
                                thread::sleep(Duration::from_millis(10))
                            }
                            _ => {
                                let _ = child.kill();
                                let _ = child.wait();
                                break;
                            }
                        }
                    }
                    *process.finished.lock().unwrap() = true;
                    process.changed.notify_all();
                })
                .expect("WPE cleanup thread");
        }
    }

    fn wait(&self) {
        let mut finished = self.finished.lock().unwrap();
        while !*finished {
            finished = self.changed.wait(finished).unwrap();
        }
    }
}

impl Drop for Worker {
    fn drop(&mut self) {
        self.outbox.close();
        self.process.stop();
    }
}

impl Worker {
    pub(super) fn for_store(store: &DataStore, gpu: Option<DmabufTarget>) -> Result<Arc<Self>> {
        if store.incognito {
            return Self::spawn(None, gpu);
        }
        let profile = match &store.directory {
            Some(directory) => directory.clone(),
            None => data_root()?.join("default"),
        };
        fs::create_dir_all(&profile).context("create browser profile")?;
        let profile = profile.canonicalize().context("resolve browser profile")?;
        let mut profiles = PROFILES.get_or_init(Mutex::default).lock().unwrap();
        profiles
            .retain(|_, (worker, process)| worker.strong_count() > 0 || process.strong_count() > 0);
        if let Some((worker, process)) = profiles.get(&profile) {
            if let Some(worker) = worker
                .upgrade()
                .filter(|worker| !worker.stopped.load(Ordering::Acquire))
            {
                return Ok(worker);
            }
            if let Some(process) = process.upgrade() {
                process.wait();
            }
        }
        let worker = Self::spawn(Some(&profile), gpu)?;
        profiles.insert(
            profile,
            (Arc::downgrade(&worker), Arc::downgrade(&worker.process)),
        );
        Ok(worker)
    }

    fn spawn(profile: Option<&Path>, gpu: Option<DmabufTarget>) -> Result<Arc<Self>> {
        let executable = helper_path()?;
        let (socket, theirs) = UnixStream::pair()?;
        let mut command = Command::new(executable);
        if let Some(profile) = profile {
            command.arg("--profile").arg(profile);
        }
        if let Some(gpu) = gpu {
            command.arg("--render-node").arg(gpu.render_node);
            if !gpu.modifiers.is_empty() {
                command.arg("--modifiers").arg(
                    gpu.modifiers
                        .iter()
                        .map(|modifier| format!("{modifier:x}"))
                        .collect::<Vec<_>>()
                        .join(","),
                );
            }
        }
        command
            .stdin(Stdio::from(OwnedFd::from(theirs)))
            .stdout(Stdio::null())
            .stderr(Stdio::inherit());
        let child = command
            .spawn()
            .context("start WPE WebKit 2.54+ helper; install the system WPE runtime and reload")?;
        drop(command);
        let process = Arc::new(Process {
            child: Mutex::new(Some(child)),
            finished: Mutex::new(false),
            changed: Condvar::new(),
        });
        socket.set_read_timeout(Some(STARTUP_TIMEOUT))?;
        let mut reader = Reader::new(socket.try_clone()?);
        let startup = (|| -> Result<()> {
            let (kind, _, bytes) = reader
                .next()
                .context("WPE helper did not start; install system WPE WebKit 2.54+ with WPEPlatform and reload the tab")?;
            if kind == b'E' {
                bail!("WPE startup failed: {}", String::from_utf8_lossy(&bytes));
            }
            ensure!(kind == b'R', "invalid WPE startup packet");
            let ready: Value = serde_json::from_slice(&bytes)?;
            let version = ready["webkit"]
                .as_str()
                .context("missing WPE runtime version")?;
            let mut parts = version.split('.').map(str::parse::<u32>);
            let major = parts.next().transpose()?.context("invalid WPE version")?;
            let minor = parts.next().transpose()?.context("invalid WPE version")?;
            ensure!(
                major > 2 || (major == 2 && minor >= 54),
                "WPE WebKit 2.54+ required; detected {version}"
            );
            Ok(())
        })();
        if let Err(error) = startup {
            let _ = socket.shutdown(Shutdown::Both);
            process.stop();
            return Err(error);
        }
        socket.set_read_timeout(None)?;
        let outbox = transport::writer(socket.try_clone()?)?;
        let routes: Arc<Mutex<HashMap<u32, Registration>>> = Arc::default();
        let stopped = Arc::new(AtomicBool::new(false));
        let worker = Arc::new(Self {
            outbox: outbox.clone(),
            routes: routes.clone(),
            stopped: stopped.clone(),
            next_id: AtomicU32::new(1),
            process: process.clone(),
        });
        thread::Builder::new()
            .name("wpe-events".into())
            .spawn(move || {
                let result = (|| -> Result<()> {
                    loop {
                        let (kind, id, data) = reader.next()?;
                        let route = routes.lock().unwrap().get(&id).and_then(|registration| {
                            registration
                                .route
                                .upgrade()
                                .map(|route| (route, registration.epoch))
                        });
                        let Some((route, epoch)) = route else {
                            if kind == b'B' {
                                reader.take_fd()?;
                            }
                            continue;
                        };
                        let _gate = route.gate.lock().unwrap();
                        if route.epoch.load(Ordering::Acquire) != epoch {
                            if kind == b'B' {
                                reader.take_fd()?;
                            }
                            continue;
                        }
                        match kind {
                            b'B' | b'X' | b'F' | b'V' | b'K' => {
                                if presentation::packet(
                                    &route,
                                    &outbox,
                                    id,
                                    kind,
                                    &data,
                                    &mut reader,
                                )?
                                .is_some()
                                {
                                    route.wake();
                                }
                            }
                            _ => route.packet(kind, &data)?,
                        }
                    }
                })();
                stopped.store(true, Ordering::Release);
                let _ = socket.shutdown(Shutdown::Both);
                let error = result.err().map_or_else(
                    || "WPE helper stopped".into(),
                    |error| format!("WPE helper stopped: {error:#}"),
                );
                let failed: Vec<_> = routes
                    .lock()
                    .unwrap()
                    .values()
                    .filter_map(|registration| {
                        registration
                            .route
                            .upgrade()
                            .map(|route| (route, registration.epoch))
                    })
                    .collect();
                for (route, epoch) in failed {
                    let _gate = route.gate.lock().unwrap();
                    if route.epoch.load(Ordering::Acquire) == epoch {
                        route.fail(error.clone());
                    }
                }
                process.stop();
            })?;
        Ok(worker)
    }

    pub(super) fn attach(self: &Arc<Self>, route: &Arc<Route>, epoch: u64) -> Result<Session> {
        let id = self
            .next_id
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |id| id.checked_add(1))
            .map_err(|_| anyhow::anyhow!("WPE page identifiers exhausted"))?;
        self.routes.lock().unwrap().insert(
            id,
            Registration {
                route: Arc::downgrade(route),
                epoch,
            },
        );
        Ok(Session {
            worker: self.clone(),
            id,
        })
    }
}

fn data_root() -> Result<PathBuf> {
    std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/share")))
        .map(|root| root.join("bezel/browser"))
        .context("no browser data directory is available")
}

fn helper_path() -> Result<PathBuf> {
    if let Some(path) = std::env::var_os("BEZEL_WPE_HELPER") {
        return Ok(path.into());
    }
    let root = std::env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".cache")))
        .context("no browser cache directory is available")?
        .join("bezel/browser");
    fs::create_dir_all(&root)?;
    fs::set_permissions(&root, fs::Permissions::from_mode(0o700))?;
    let hash = format!("{:x}", Sha256::digest(HELPER));
    let path = root.join(format!("wpe-{hash}"));
    if fs::read(&path).ok().as_deref() == Some(HELPER) {
        return Ok(path);
    }
    let staging = root.join(format!(
        ".wpe-{}-{}-{}",
        std::process::id(),
        EXTRACTION.fetch_add(1, Ordering::Relaxed),
        hash
    ));
    let result = (|| -> io::Result<()> {
        use std::io::Write as _;
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o700)
            .open(&staging)?;
        file.write_all(HELPER)?;
        file.sync_all()?;
        fs::rename(&staging, &path)
    })();
    if result.is_err() {
        let _ = fs::remove_file(&staging);
    }
    result.context("extract WPE helper")?;
    Ok(path)
}

pub(super) fn store_request(store: &DataStore, command: &str) -> Result<Value> {
    let worker = Worker::for_store(store, None)?;
    if command == "store-clear" {
        ensure!(
            !worker
                .routes
                .lock()
                .unwrap()
                .values()
                .filter_map(|registration| registration.route.upgrade())
                .any(|route| route.page),
            "close browser pages before clearing their store"
        );
    }
    let (answer, result) = std::sync::mpsc::sync_channel(1);
    let route = Arc::new(Route::store(answer));
    let session = worker.attach(&route, route.epoch.load(Ordering::Acquire))?;
    worker
        .outbox
        .send(session.id, json!({"cmd":command}))
        .map_err(anyhow::Error::msg)?;
    result
        .recv_timeout(STARTUP_TIMEOUT)
        .context("WPE storage operation did not finish")
}

impl Drop for Process {
    fn drop(&mut self) {
        if let Some(mut child) = self.child.get_mut().unwrap().take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}
