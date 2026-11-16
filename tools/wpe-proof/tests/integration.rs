#![cfg(target_os = "linux")]
use gpui::{
    AppContext, Bounds, Context, DevicePixels, HeadlessAppContext, IntoElement, Render,
    RenderImage, Styled, Window, external_image, img, point, px, size,
};
use std::{
    io::{BufRead, BufReader, Read, Write},
    os::unix::process::CommandExt,
    path::{Path, PathBuf},
    process::{Child, ChildStdin, Command, Stdio},
    sync::{Arc, Mutex, mpsc},
    thread,
    time::{Duration, Instant},
};

struct Helper {
    child: Child,
    executable: PathBuf,
    input: Arc<Mutex<ChildStdin>>,
    output: mpsc::Receiver<Message>,
}
#[derive(Debug)]
enum Message {
    Frame {
        id: u32,
        width: i32,
        height: i32,
        bytes: Vec<u8>,
    },
    State(String),
    Released(u32),
    Held(u32),
}
impl Helper {
    fn start(libraries: Option<&Path>) -> Self {
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let executable =
            std::env::temp_dir().join(format!("bezel-independent-wpe-{}", std::process::id()));
        let flags = Command::new("pkg-config")
            .args([
                "--cflags",
                "--libs",
                "wpe-webkit-2.0 >= 2.54",
                "wpe-platform-headless-2.0",
            ])
            .output()
            .unwrap();
        assert!(
            flags.status.success(),
            "{}",
            String::from_utf8_lossy(&flags.stderr)
        );
        let status = Command::new("cc")
            .args([
                "-std=gnu11",
                "-Wall",
                "-Wextra",
                "-Werror",
                "-Wno-unused-parameter",
            ])
            .arg(dir.join("tests/platform.c"))
            .arg("-o")
            .arg(&executable)
            .args(String::from_utf8(flags.stdout).unwrap().split_whitespace())
            .status()
            .unwrap();
        assert!(status.success());
        let mut command = Command::new("dbus-run-session");
        if let Some(libraries) = libraries {
            command.env("LD_LIBRARY_PATH", libraries);
        }
        let mut child = command
            .arg("--")
            .arg(&executable)
            .env("G_DEBUG", "fatal-criticals")
            .process_group(0)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        let input = Arc::new(Mutex::new(child.stdin.take().unwrap()));
        let mut stdout = BufReader::new(child.stdout.take().unwrap());
        let (sender, output) = mpsc::channel();
        thread::spawn(move || {
            loop {
                let mut line = String::new();
                if stdout.read_line(&mut line).unwrap() == 0 {
                    break;
                }
                let fields: Vec<_> = line.split_whitespace().collect();
                let message = match fields[0] {
                    "F" => {
                        let id = fields[1].parse().unwrap();
                        let width: i32 = fields[2].parse().unwrap();
                        let height: i32 = fields[3].parse().unwrap();
                        let length: usize = fields[4].parse().unwrap();
                        assert_eq!(length, (width * height * 4) as usize);
                        assert!(length <= 320 * 200 * 4);
                        let mut bytes = vec![0; length];
                        stdout.read_exact(&mut bytes).unwrap();
                        Message::Frame {
                            id,
                            width,
                            height,
                            bytes,
                        }
                    }
                    "S" => Message::State(line[2..].trim().to_owned()),
                    "R" => Message::Released(fields[1].parse().unwrap()),
                    "H" => Message::Held(fields[1].parse().unwrap()),
                    _ => panic!("Unexpected helper output: {line}"),
                };
                if sender.send(message).is_err() {
                    break;
                }
            }
        });
        Self {
            child,
            executable,
            input,
            output,
        }
    }
    fn send(&self, command: &str) {
        writeln!(self.input.lock().unwrap(), "{command}").unwrap();
    }
    fn receive(&self) -> Result<Message, mpsc::RecvTimeoutError> {
        self.output.recv_timeout(Duration::from_secs(30))
    }
}
impl Drop for Helper {
    fn drop(&mut self) {
        // The D-Bus launcher and WPE helper share this owned process group.
        if self.child.try_wait().is_ok_and(|status| status.is_none()) {
            unsafe {
                libc::killpg(self.child.id() as libc::pid_t, libc::SIGKILL);
            }
        }
        let _ = self.child.wait();
        let _ = std::fs::remove_file(&self.executable);
    }
}
struct Page {
    image: Arc<RenderImage>,
    zoom: f32,
}
impl Render for Page {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        img(self.image.clone())
            .w(px(320.0 * self.zoom))
            .h(px(200.0 * self.zoom))
    }
}
#[test]
fn wpe_frames_input_zoom_and_gpu_release() {
    let mut helper = Helper::start(None);
    let platform = gpui_platform::current_platform(true);
    let mut cx = HeadlessAppContext::with_platform(
        platform.text_system(),
        Arc::new(()),
        gpui_platform::current_headless_renderer,
    );
    let image = Arc::new(RenderImage::blank(size(DevicePixels(320), DevicePixels(200))).unwrap());
    let handle = cx
        .open_window(size(px(640.0), px(400.0)), |_, cx| {
            cx.new(|_| Page {
                image: image.clone(),
                zoom: 1.0,
            })
        })
        .unwrap();
    let mut state = String::new();
    let mut colors = Vec::new();
    let mut releases = Vec::new();
    let mut phase = 0;
    let mut latest = None;
    let deadline = Instant::now() + Duration::from_secs(50);
    loop {
        assert!(
            Instant::now() < deadline,
            "state={state}, colors={colors:?}"
        );
        match helper.receive().expect("WPE helper response") {
            Message::State(value) => {
                state = value;
            }
            Message::Released(id) => releases.push(id),
            Message::Held(_) => panic!("Unexpected ownership reply"),
            Message::Frame {
                id,
                width,
                height,
                bytes,
            } => {
                assert_eq!((width, height), (320, 200));
                helper.send("held");
                loop {
                    match helper.receive().expect("WPE helper response") {
                        Message::Held(count) => {
                            assert_eq!(count, 1);
                            break;
                        }
                        Message::State(value) => state = value,
                        Message::Released(released) => {
                            assert_ne!(released, id, "buffer released before GPU submission");
                            releases.push(released);
                        }
                        Message::Frame { .. } => panic!("Producer rendered before acknowledgment"),
                    }
                }
                let (done, completed) = mpsc::channel();
                let input = helper.input.clone();
                cx.update_window(handle.into(), |_, window, cx| {
                    window.insert_blank_image(&image, 0).unwrap();
                    assert!(
                        window
                            .update_image(
                                &image,
                                0,
                                Bounds::new(
                                    point(DevicePixels(0), DevicePixels(0)),
                                    size(DevicePixels(width), DevicePixels(height))
                                ),
                                external_image::AtlasTileUpdate::Pixels {
                                    bytes: &bytes,
                                    bytes_per_row: width as u32 * 4
                                },
                                Some(Box::new(move || {
                                    writeln!(input.lock().unwrap(), "release {id}").unwrap();
                                    done.send(()).unwrap();
                                }))
                            )
                            .unwrap()
                    );
                    window.draw(cx).clear(cx);
                })
                .unwrap();
                let screenshot = cx.capture_screenshot(handle.into()).unwrap();
                completed
                    .recv_timeout(Duration::from_secs(10))
                    .expect("GPU completion");
                let scale = screenshot.width() as f32 / 640.0;
                let zoom = if phase == 0 { 1.0 } else { 2.0 };
                let pixel = screenshot
                    .get_pixel((250.0 * zoom * scale) as u32, (150.0 * zoom * scale) as u32)
                    .0;
                colors.push(pixel);
                latest = Some((id, pixel));
            }
        }
        if let Some((id, pixel)) = latest {
            match phase {
                0 if pixel == [224, 32, 48, 255] && state == "320:200:0::0" => {
                    handle
                        .update(&mut cx, |page, _, cx| {
                            page.zoom = 2.0;
                            cx.notify();
                        })
                        .unwrap();
                    cx.update_window(handle.into(), |_, window, cx| {
                        window.draw(cx).clear(cx);
                    })
                    .unwrap();
                    let zoomed = cx.capture_screenshot(handle.into()).unwrap();
                    let scale = zoomed.width() as f32 / 640.0;
                    assert_eq!(
                        zoomed
                            .get_pixel((500.0 * scale) as u32, (300.0 * scale) as u32)
                            .0,
                        pixel
                    );
                    helper.send("click 50 40");
                    phase = 1;
                }
                1 if pixel == [32, 224, 48, 255] && state == "320:200:1::0" => {
                    helper.send("click 50 100");
                    phase = 2;
                }
                2 if state == "320:200:1::1" => {
                    helper.send("key 38 97");
                    phase = 3;
                }
                3 if pixel == [32, 48, 224, 255] && state == "320:200:1:a:1" => {
                    while !releases.contains(&id) {
                        match helper.receive().expect("WPE helper response") {
                            Message::Released(value) => releases.push(value),
                            Message::State(value) => state = value,
                            Message::Frame { id, .. } => helper.send(&format!("release {id}")),
                            Message::Held(_) => (),
                        }
                    }
                    helper.send("quit");
                    assert!(helper.child.wait().unwrap().success());
                    println!(
                        "WPE real page -> GPUI Vulkan: {state}; frames={}, released={}",
                        colors.len(),
                        releases.len()
                    );
                    break;
                }
                _ => (),
            }
        }
    }
}

#[test]
fn incompatible_wpe_is_confined_to_the_helper() {
    let dependencies = Command::new("ldd")
        .arg(std::env::current_exe().unwrap())
        .output()
        .unwrap();
    assert!(dependencies.status.success());
    assert!(!String::from_utf8_lossy(&dependencies.stdout).contains("libWPEWebKit"));
    let directory =
        std::env::temp_dir().join(format!("bezel-incompatible-wpe-{}", std::process::id()));
    std::fs::create_dir(&directory).unwrap();
    std::fs::write(
        directory.join("libWPEWebKit-2.0.so.1"),
        b"incompatible runtime",
    )
    .unwrap();
    let mut helper = Helper::start(Some(&directory));
    assert!(matches!(
        helper.receive(),
        Err(mpsc::RecvTimeoutError::Disconnected)
    ));
    assert!(!helper.child.wait().unwrap().success());
    std::fs::remove_dir_all(directory).unwrap();
}
