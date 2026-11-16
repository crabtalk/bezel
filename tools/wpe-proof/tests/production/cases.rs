use crate::page::platform::{
    presentation::{Configuration, Presenter, Received, Surface},
    transport::{Outbox, Packet, Reader},
};
use gpui::{
    AppContext, Context, HeadlessAppContext, IntoElement, Render, RenderImage, Styled, Window,
    canvas, native_view::Viewport, px, size,
};
use serde_json::{Value, json};
use std::{
    cell::RefCell,
    fs,
    io::{Read as _, Write as _},
    net::TcpListener,
    os::{fd::OwnedFd, unix::net::UnixStream},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    rc::Rc,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
        mpsc,
    },
    thread,
    time::{Duration, Instant},
};

static ENGINE_ID: AtomicUsize = AtomicUsize::new(0);

struct Engine {
    child: Child,
    path: PathBuf,
    commands: Outbox,
    messages: mpsc::Receiver<Packet>,
}
impl Engine {
    fn new(profile: Option<&Path>) -> Self {
        let source = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../crates/browser/src/page/linux/helper.c");
        let path = std::env::temp_dir().join(format!(
            "bezel-production-wpe-{}-{}",
            std::process::id(),
            ENGINE_ID.fetch_add(1, Ordering::Relaxed)
        ));
        let flags = Command::new("pkg-config")
            .args([
                "--cflags",
                "--libs",
                "wpe-webkit-2.0 >= 2.54",
                "wpe-platform-headless-2.0",
                "json-glib-1.0",
                "xkbcommon",
            ])
            .output()
            .unwrap();
        assert!(flags.status.success());
        assert!(
            Command::new("cc")
                .args([
                    "-std=gnu11",
                    "-Wall",
                    "-Wextra",
                    "-Werror",
                    "-Wno-unused-parameter"
                ])
                .arg(source)
                .arg("-o")
                .arg(&path)
                .args(String::from_utf8(flags.stdout).unwrap().split_whitespace())
                .status()
                .unwrap()
                .success()
        );
        let (host, guest) = UnixStream::pair().unwrap();
        host.set_read_timeout(Some(Duration::from_secs(20)))
            .unwrap();
        let mut command = Command::new(&path);
        if let Some(profile) = profile {
            command.arg("--profile").arg(profile);
        }
        let child = command
            .stdin(Stdio::from(OwnedFd::from(guest)))
            .env("G_DEBUG", "fatal-criticals")
            .spawn()
            .unwrap();
        let commands = Outbox::new(host.try_clone().unwrap()).unwrap();
        let (sender, messages) = mpsc::channel();
        thread::spawn(move || {
            let mut reader = Reader::new(host);
            while let Ok(packet) = reader.next() {
                if sender.send(packet).is_err() {
                    break;
                }
            }
        });
        let engine = Self {
            child,
            path,
            commands,
            messages,
        };
        let Packet { kind, bytes, .. } = engine.next();
        assert_eq!(kind, b'R', "{}", String::from_utf8_lossy(&bytes));
        assert!(
            serde_json::from_slice::<Value>(&bytes).unwrap()["webkit"]
                .as_str()
                .unwrap()
                .starts_with("2.54.")
        );
        engine
    }
    fn next(&self) -> Packet {
        self.messages
            .recv_timeout(Duration::from_secs(20))
            .expect("production helper response")
    }
    fn send(&self, id: u32, command: Value) {
        self.commands.send(id, command).unwrap();
    }
}
impl Drop for Engine {
    fn drop(&mut self) {
        self.commands.close();
        let deadline = Instant::now() + Duration::from_secs(3);
        while self.child.try_wait().unwrap().is_none() && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(10));
        }
        if self.child.try_wait().unwrap().is_none() {
            let _ = self.child.kill();
        }
        let _ = self.child.wait();
        let _ = fs::remove_file(&self.path);
    }
}

struct Scene {
    surface: Arc<Mutex<Surface>>,
    configuration: Configuration,
    presenter: Rc<RefCell<Presenter>>,
    image: Rc<RefCell<Option<Arc<RenderImage>>>>,
    zoom: f32,
}
impl Render for Scene {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let surface = self.surface.clone();
        let configuration = self.configuration;
        let presenter = self.presenter.clone();
        let image = self.image.clone();
        canvas(
            |_, _, _| (),
            move |bounds, _, window, _| {
                presenter
                    .borrow_mut()
                    .paint(
                        &surface,
                        Some(configuration),
                        &mut image.borrow_mut(),
                        bounds,
                        window,
                    )
                    .unwrap();
            },
        )
        .w(px(f32::from(configuration.viewport.width) * self.zoom))
        .h(px(f32::from(configuration.viewport.height) * self.zoom))
    }
}

#[test]
fn helper_navigation_storage_input_snapshot_and_shutdown() {
    let directory =
        std::env::temp_dir().join(format!("bezel-production-profile-{}", std::process::id()));
    fs::create_dir(&directory).unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}/", listener.local_addr().unwrap());
    thread::spawn(move || {
        for mut stream in listener.incoming().flatten() {
            let mut request = [0; 4096];
            let _ = stream.read(&mut request);
            let body = "<!doctype html><style>html{background:#e02030}input{position:absolute;left:20px;top:60px;width:120px;height:40px}</style><button onclick=\"document.title='clicked';document.documentElement.style.background='#20e030'\">Click</button><script>let keyCount=0;addEventListener('keydown',e=>{if(e.key==='a')++keyCount})</script><input onfocus=\"document.title='focused'\" oninput=\"document.title=this.value\">";
            let _ = write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            );
        }
    });
    let mut engine = Engine::new(Some(&directory));
    engine.send(1, json!({"cmd":"create","script":"","userAgent":""}));
    engine.send(
        1,
        json!({"cmd":"resize","width":320,"height":200,"scale":1,"generation":1}),
    );
    engine.send(1, json!({"cmd":"active","value":1}));
    engine.send(1, json!({"cmd":"focus","value":1}));
    engine.send(1, json!({"cmd":"load","url":url}));
    let surface = Arc::new(Mutex::new(Surface::default()));
    let configuration = Configuration {
        viewport: Viewport::try_from(size(px(320.0), px(200.0))).unwrap(),
        scale: 1.0,
        generation: 1,
    };
    let presenter = Rc::new(RefCell::new(Presenter::new(engine.commands.clone(), 1)));
    let image = Rc::new(RefCell::new(None));
    presenter.borrow_mut().sync_focus(true, true);
    let mut cx = HeadlessAppContext::with_platform(
        gpui_platform::current_platform(true).text_system(),
        Arc::new(()),
        gpui_platform::current_headless_renderer,
    );
    let handle = cx
        .open_window(size(px(640.0), px(400.0)), |_, cx| {
            cx.new(|_| Scene {
                surface: surface.clone(),
                configuration,
                presenter: presenter.clone(),
                image: image.clone(),
                zoom: 1.0,
            })
        })
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(45);
    let mut loaded = false;
    let mut painted = false;
    let mut buffers = 0;
    while !(loaded && painted) {
        assert!(Instant::now() < deadline, "initial frame stalled");
        let packet = engine.next();
        match packet.kind {
            b'E' => panic!("{}", String::from_utf8_lossy(&packet.bytes)),
            b'B' | b'F' | b'K' | b'X' => {
                let frame = packet.kind == b'F';
                surface
                    .lock()
                    .unwrap()
                    .receive(Some(configuration), &engine.commands, packet)
                    .unwrap();
                if frame {
                    buffers += 1;
                    cx.update_window(handle.into(), |_, window, cx| window.draw(cx).clear(cx))
                        .unwrap();
                    let screenshot = cx.capture_screenshot(handle.into()).unwrap();
                    let scale = screenshot.width() as f32 / 640.0;
                    painted = screenshot
                        .get_pixel((250.0 * scale) as u32, (150.0 * scale) as u32)
                        .0
                        == [224, 32, 48, 255];
                }
            }
            b'S' => {
                let state: Value = serde_json::from_slice(&packet.bytes).unwrap();
                loaded = state["url"] == url && state["loading"] == false;
            }
            _ => (),
        }
    }
    let shown = presenter.borrow().snapshot_frame().unwrap();
    assert_eq!(shown.1, 1);
    handle
        .update(&mut cx, |scene, _, cx| {
            scene.zoom = 2.0;
            cx.notify()
        })
        .unwrap();
    cx.update_window(handle.into(), |_, window, cx| window.draw(cx).clear(cx))
        .unwrap();
    let zoomed = cx.capture_screenshot(handle.into()).unwrap();
    let scale = zoomed.width() as f32 / 640.0;
    assert_eq!(
        zoomed
            .get_pixel((500.0 * scale) as u32, (300.0 * scale) as u32)
            .0,
        [224, 32, 48, 255]
    );
    engine.send(1,json!({"cmd":"eval","request":1,"script":"localStorage.setItem('bezel','persisted');document.cookie='bezel=stored;max-age=3600';JSON.stringify({width:innerWidth,height:innerHeight})"}));
    let mut evaluated = false;
    engine.send(1, json!({"cmd":"snapshot","request":2}));
    let mut captured = false;
    while !(evaluated && captured) {
        let packet = engine.next();
        let kind = packet.kind;
        let bytes = packet.bytes.clone();
        let info: Value = if matches!(kind, b'J' | b'P' | b'Q' | b'F') {
            serde_json::from_slice(&bytes).unwrap()
        } else {
            Value::Null
        };
        match kind {
            b'E' => panic!("{}", String::from_utf8_lossy(&bytes)),
            b'J' => {
                assert!(info["value"].as_str().unwrap().contains("320"));
                evaluated = true;
            }
            b'P' => {
                match surface
                    .lock()
                    .unwrap()
                    .receive(Some(configuration), &engine.commands, packet)
                    .unwrap()
                {
                    Received::Snapshot { request, image } => {
                        assert_eq!(request, Some(2));
                        assert_eq!(image.size(0).width.0, 320);
                        captured = true;
                    }
                    _ => panic!("Missing decoded snapshot"),
                }
            }
            b'Q' => panic!("WPE snapshot failed"),
            b'F' => {
                buffers += 1;
                engine.send(1, json!({"cmd":"presented","buffer":info["buffer"]}));
                engine.send(1, json!({"cmd":"released","buffer":info["buffer"]}));
            }
            _ => (),
        }
    }
    engine.send(
        1,
        json!({"cmd":"down","x":15,"y":15,"button":1,"count":1,"mods":0}),
    );
    engine.send(
        1,
        json!({"cmd":"up","x":15,"y":15,"button":1,"count":0,"mods":0}),
    );
    let mut phase = 0;
    let mut verified = false;
    let mut green = false;
    while !(verified && green) {
        assert!(Instant::now() < deadline, "input phase {phase} stalled");
        let packet = engine.next();
        match packet.kind {
            b'E' => panic!("{}", String::from_utf8_lossy(&packet.bytes)),
            b'B' | b'F' | b'K' | b'X' => {
                let frame = packet.kind == b'F';
                surface
                    .lock()
                    .unwrap()
                    .receive(Some(configuration), &engine.commands, packet)
                    .unwrap();
                if frame {
                    cx.update_window(handle.into(), |_, window, cx| window.draw(cx).clear(cx))
                        .unwrap();
                    let screenshot = cx.capture_screenshot(handle.into()).unwrap();
                    let scale = screenshot.width() as f32 / 640.0;
                    green = screenshot
                        .get_pixel((500.0 * scale) as u32, (300.0 * scale) as u32)
                        .0
                        == [32, 224, 48, 255];
                }
            }
            b'S' => {
                let state: Value = serde_json::from_slice(&packet.bytes).unwrap();
                match (phase, state["title"].as_str()) {
                    (0, Some("clicked")) => {
                        engine.send(
                            1,
                            json!({"cmd":"down","x":50,"y":80,"button":1,"count":1,"mods":0}),
                        );
                        engine.send(
                            1,
                            json!({"cmd":"up","x":50,"y":80,"button":1,"count":0,"mods":0}),
                        );
                        phase = 1;
                    }
                    (1, Some("focused")) => {
                        engine.send(1, json!({"cmd":"key_down","key":"a","mods":0}));
                        engine.send(1, json!({"cmd":"key_up","key":"a","mods":0}));
                        phase = 2;
                    }
                    (2, Some("a")) => {
                        engine.send(1, json!({"cmd":"preedit","text":"é"}));
                        engine.send(1, json!({"cmd":"commit","text":"é"}));
                        phase = 3;
                    }
                    (3, Some("aé")) => {
                        engine.send(1,json!({"cmd":"eval","request":3,"script":"JSON.stringify({text:document.querySelector('input').value,keys:keyCount,width:innerWidth})"}));
                        phase = 4;
                    }
                    _ => (),
                }
            }
            b'J' => {
                let reply: Value = serde_json::from_slice(&packet.bytes).unwrap();
                if reply["request"] == 3 {
                    let encoded: String =
                        serde_json::from_str(reply["value"].as_str().unwrap()).unwrap();
                    let answer: Value = serde_json::from_str(&encoded).unwrap();
                    assert_eq!(answer, json!({"text":"aé","keys":1,"width":320}));
                    verified = true;
                }
            }
            _ => (),
        }
    }
    let configuration = Configuration {
        viewport: Viewport::try_from(size(px(400.0), px(240.0))).unwrap(),
        scale: 1.0,
        generation: 2,
    };
    handle
        .update(&mut cx, |scene, _, cx| {
            scene.configuration = configuration;
            scene.zoom = 1.0;
            cx.notify();
        })
        .unwrap();
    engine.send(1, configuration.command());
    engine.send(1, json!({"cmd":"visible","value":0}));
    engine.send(1, json!({"cmd":"visible","value":1}));
    loop {
        let packet = engine.next();
        let kind = packet.kind;
        if kind == b'E' {
            panic!("{}", String::from_utf8_lossy(&packet.bytes));
        }
        if matches!(kind, b'B' | b'F' | b'K' | b'X') {
            let generation = if kind == b'F' {
                serde_json::from_slice::<Value>(&packet.bytes).unwrap()["generation"].as_u64()
            } else {
                None
            };
            let event = surface
                .lock()
                .unwrap()
                .receive(Some(configuration), &engine.commands, packet)
                .unwrap();
            if kind == b'F' && generation == Some(1) {
                assert!(matches!(event, Received::Unchanged));
            }
            if kind == b'F' && generation == Some(2) && matches!(event, Received::Changed) {
                cx.update_window(handle.into(), |_, window, cx| window.draw(cx).clear(cx))
                    .unwrap();
                cx.capture_screenshot(handle.into()).unwrap();
                assert_eq!(presenter.borrow().snapshot_frame().unwrap().1, 2);
                assert_eq!(
                    image.borrow().as_ref().unwrap().size(0),
                    size(gpui::DevicePixels(400), gpui::DevicePixels(240))
                );
                break;
            }
        }
    }
    drop(cx);
    drop(presenter);
    drop(image);
    surface.lock().unwrap().clear();
    engine.send(1, json!({"cmd":"close"}));
    engine.send(2, json!({"cmd":"store-usage"}));
    loop {
        let Packet { kind, bytes, .. } = engine.next();
        if kind == b'D' {
            assert!(serde_json::from_slice::<Value>(&bytes).unwrap()["sites"].is_array());
            break;
        }
    }
    engine.commands.close();
    assert!(engine.child.wait().unwrap().success());
    assert!(buffers > 0);
    drop(engine);
    for (round, persistent) in [false, true, true].into_iter().enumerate() {
        let mut engine = Engine::new(persistent.then_some(directory.as_path()));
        engine.send(1, json!({"cmd":"create","script":"","userAgent":""}));
        engine.send(1, configuration.command());
        engine.send(1, json!({"cmd":"load","url":url}));
        let mut loaded = false;
        let deadline = Instant::now() + Duration::from_secs(45);
        loop {
            assert!(Instant::now() < deadline, "storage navigation stalled");
            let packet = engine.next();
            match packet.kind {
                b'E' => panic!("{}", String::from_utf8_lossy(&packet.bytes)),
                b'F' => {
                    let info: Value = serde_json::from_slice(&packet.bytes).unwrap();
                    engine.send(1, json!({"cmd":"presented","buffer":info["buffer"]}));
                    engine.send(1, json!({"cmd":"released","buffer":info["buffer"]}));
                }
                b'S' if !loaded => {
                    let state: Value = serde_json::from_slice(&packet.bytes).unwrap();
                    if state["url"] == url && state["loading"] == false {
                        loaded = true;
                        engine.send(1,json!({"cmd":"eval","request":4,"script":"JSON.stringify({stored:localStorage.getItem('bezel'),cookie:document.cookie})"}));
                    }
                }
                b'J' => {
                    let reply: Value = serde_json::from_slice(&packet.bytes).unwrap();
                    let encoded: String =
                        serde_json::from_str(reply["value"].as_str().unwrap()).unwrap();
                    let answer: Value = serde_json::from_str(&encoded).unwrap();
                    if persistent && round == 1 {
                        assert_eq!(answer["stored"], "persisted");
                        assert!(answer["cookie"].as_str().unwrap().contains("bezel=stored"));
                    } else {
                        assert!(answer["stored"].is_null());
                        assert_eq!(answer["cookie"], "");
                    }
                    break;
                }
                _ => (),
            }
        }
        engine.send(1, json!({"cmd":"close"}));
        engine.send(2, json!({"cmd":"store-clear"}));
        loop {
            let packet = engine.next();
            if packet.kind == b'D' {
                assert_eq!(
                    serde_json::from_slice::<Value>(&packet.bytes).unwrap()["ok"],
                    true
                );
                break;
            }
        }
        engine.commands.close();
        assert!(engine.child.wait().unwrap().success());
    }
    fs::remove_dir_all(directory).unwrap();
}
