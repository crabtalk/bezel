//! A scripted terminal session — the terminal emulator fed a canned
//! ANSI byte stream on a timer, because a gallery page has no PTY to point at
//! anything real.
//!
//! **The PTY is the host's job by design.** terminal is bytes in, grid
//! out: the escape-sequence state machine and the paint, nothing else. This
//! page is the host — it plays the script below, hands the emulator's
//! snapshots to [`TerminalElement`], then scrolls back through history and
//! replays. Copy this file; your bytes come from wherever your session runs.
//!
//! Native-only: `alacritty_terminal` pulls `home`, which does not compile for
//! wasm32, so the crate (and this page) sits off the web build.

use gpui::{Context, Render, SharedString, Subscription, Task, Window, div, prelude::*, px};
use std::time::Duration;
use terminal::{
    emulator::Emulator,
    view::{GridSnapshot, TerminalElement, terminal_panel_bg},
};
use theme::{Theme, hairline};

/// Timer tick, driving the wall-clock script playback.
const TICK_MS: u64 = 80;
/// After the script's last byte: how long before the demo scrolls back into
/// history, and how long before it restarts.
const SCROLL_AT_MS: u64 = 1_200;
const RESTART_AT_MS: u64 = 3_000;

/// One beat of the script: bytes, and the delay after the previous beat.
/// Cumulative times are computed once at startup, not hand-written.
///
/// The session: an OSC title, a colored prompt, `cargo build` typed out with a
/// warning (box-drawing chars included — they exercise the grid's font-fallback
/// pinning), then a `tree crates` listing long enough to leave scrollback.
const SCRIPT: &[(&[u8], u64)] = &[
    (b"\x1b]0;bezel \xe2\x80\x94 cargo build\x07", 300),
    (b"\x1b[1;32mclearloop@bezel\x1b[0m:\x1b[1;34m~/code/bezel\x1b[0m$ ", 450),
    (b"c", 90),
    (b"a", 90),
    (b"r", 90),
    (b"g", 90),
    (b"o", 90),
    (b" ", 90),
    (b"b", 90),
    (b"u", 90),
    (b"i", 90),
    (b"l", 90),
    (b"d", 90),
    (b"\r\n", 220),
    (b"\x1b[1;32m   Compiling\x1b[0m theme v0.0.2\r\n", 340),
    (b"\x1b[1;32m   Compiling\x1b[0m motion v0.0.2\r\n", 260),
    (b"\x1b[1;32m   Compiling\x1b[0m terminal v0.0.2\r\n", 260),
    (b"\x1b[1;33m    Warning\x1b[0m: unused variable: `i`\r\n", 340),
    (b"\x1b[1;33m   --> crates/theme/src/color.rs:142:9\x1b[0m\r\n", 170),
    (b"\x1b[1;33m    \xe2\x94\x82\x1b[0m\r\n", 170),
    (b"\x1b[1;33m142 \xe2\x94\x82\x1b[0m     let i = 42;\r\n", 170),
    (b"\x1b[1;33m    \xe2\x94\x82\x1b[0m         ^ unused\r\n", 170),
    (b"\x1b[1;32m    Finished\x1b[0m `dev` profile [unoptimized + debuginfo] target(s) in 12.4s\r\n", 420),
    (b"\x1b[1;32mclearloop@bezel\x1b[0m:\x1b[1;34m~/code/bezel\x1b[0m$ ", 520),
    (b"t", 90),
    (b"r", 90),
    (b"e", 90),
    (b"e", 90),
    (b" ", 90),
    (b"c", 90),
    (b"r", 90),
    (b"a", 90),
    (b"t", 90),
    (b"e", 90),
    (b"s", 90),
    (b"\r\n", 220),
    (b"\x1b[1;34mcrates\x1b[0m\r\n", 110),
    (b"\xe2\x94\x9c\xe2\x94\x80\xe2\x94\x80 \x1b[1;34mbezel\x1b[0m\r\n", 90),
    (b"\xe2\x94\x82   \xe2\x94\x94\xe2\x94\x80\xe2\x94\x80 src\r\n", 90),
    (b"\xe2\x94\x82       \xe2\x94\x94\xe2\x94\x80\xe2\x94\x80 lib.rs\r\n", 90),
    (b"\xe2\x94\x9c\xe2\x94\x80\xe2\x94\x80 \x1b[1;34mmarkdown\x1b[0m\r\n", 90),
    (b"\xe2\x94\x82   \xe2\x94\x94\xe2\x94\x80\xe2\x94\x80 src\r\n", 90),
    (b"\xe2\x94\x82       \xe2\x94\x94\xe2\x94\x80\xe2\x94\x80 lib.rs\r\n", 90),
    (b"\xe2\x94\x9c\xe2\x94\x80\xe2\x94\x80 \x1b[1;34mmotion\x1b[0m\r\n", 90),
    (b"\xe2\x94\x82   \xe2\x94\x94\xe2\x94\x80\xe2\x94\x80 src\r\n", 90),
    (b"\xe2\x94\x82       \xe2\x94\x94\xe2\x94\x80\xe2\x94\x80 lib.rs\r\n", 90),
    (b"\xe2\x94\x9c\xe2\x94\x80\xe2\x94\x80 \x1b[1;34mterminal\x1b[0m\r\n", 90),
    (b"\xe2\x94\x82   \xe2\x94\x9c\xe2\x94\x80\xe2\x94\x80 src\r\n", 90),
    (b"\xe2\x94\x82   \xe2\x94\x82   \xe2\x94\x9c\xe2\x94\x80\xe2\x94\x80 emulator.rs\r\n", 90),
    (b"\xe2\x94\x82   \xe2\x94\x82   \xe2\x94\x9c\xe2\x94\x80\xe2\x94\x80 lib.rs\r\n", 90),
    (b"\xe2\x94\x82   \xe2\x94\x82   \xe2\x94\x94\xe2\x94\x80\xe2\x94\x80 view.rs\r\n", 90),
    (b"\xe2\x94\x82   \xe2\x94\x94\xe2\x94\x80\xe2\x94\x80 tests\r\n", 90),
    (b"\xe2\x94\x82       \xe2\x94\x9c\xe2\x94\x80\xe2\x94\x80 emulator.rs\r\n", 90),
    (b"\xe2\x94\x82       \xe2\x94\x94\xe2\x94\x80\xe2\x94\x80 view.rs\r\n", 90),
    (b"\xe2\x94\x9c\xe2\x94\x80\xe2\x94\x80 \x1b[1;34mtheme\x1b[0m\r\n", 90),
    (b"\xe2\x94\x82   \xe2\x94\x94\xe2\x94\x80\xe2\x94\x80 src\r\n", 90),
    (b"\xe2\x94\x82       \xe2\x94\x9c\xe2\x94\x80\xe2\x94\x80 appearance.rs\r\n", 90),
    (b"\xe2\x94\x82       \xe2\x94\x9c\xe2\x94\x80\xe2\x94\x80 color.rs\r\n", 90),
    (b"\xe2\x94\x82       \xe2\x94\x9c\xe2\x94\x80\xe2\x94\x80 lib.rs\r\n", 90),
    (b"\xe2\x94\x82       \xe2\x94\x94\xe2\x94\x80\xe2\x94\x80 paint.rs\r\n", 90),
    (b"\xe2\x94\x94\xe2\x94\x80\xe2\x94\x80 \x1b[1;34mui\x1b[0m\r\n", 90),
    (b"    \xe2\x94\x94\xe2\x94\x80\xe2\x94\x80 src\r\n", 90),
    (b"        \xe2\x94\x9c\xe2\x94\x80\xe2\x94\x80 popover.rs\r\n", 90),
    (b"        \xe2\x94\x9c\xe2\x94\x80\xe2\x94\x80 table.rs\r\n", 90),
    (b"        \xe2\x94\x94\xe2\x94\x80\xe2\x94\x80 tree.rs\r\n", 90),
];

pub struct Terminal {
    emulator: Emulator,
    /// The script with cumulative arrival times (ms).
    script: Vec<(&'static [u8], u64)>,
    /// Playback position, advanced one [`TICK_MS`] per tick that actually
    /// fired. A page nobody is looking at does not tick, so the session pauses
    /// where it stood rather than replaying into the void.
    elapsed: u64,
    /// Script beats fed so far.
    fed: usize,
    /// The history-scroll phase has happened.
    scrolled: bool,
    /// Pending tick. At most one in flight; dropping it pauses the session.
    tick: Option<Task<()>>,
    /// Registered lazily on first render, since it needs a `Window`.
    activation: Option<Subscription>,
}

impl Terminal {
    pub fn new(_: &mut Context<Self>) -> Self {
        let script = {
            let mut at = 0;
            SCRIPT
                .iter()
                .map(|(bytes, delay)| {
                    at += delay;
                    (*bytes, at)
                })
                .collect()
        };
        Self {
            emulator: Emulator::new(80, 24),
            script,
            elapsed: 0,
            fed: 0,
            scrolled: false,
            tick: None,
            activation: None,
        }
    }

    /// Keep the timer already in flight rather than cancel and reschedule it —
    /// a render between ticks (a click, a scroll) would otherwise push the next
    /// beat farther out every time and stall playback.
    fn schedule_tick(&mut self, cx: &mut Context<Self>) {
        if self.tick.is_some() {
            return;
        }
        self.tick = Some(cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(TICK_MS))
                .await;
            let _ = this.update(cx, |this, cx| {
                this.tick = None;
                this.elapsed += TICK_MS;
                this.play(cx);
            });
        }));
    }

    /// Play the script from the tick clock: every beat whose time has come is
    /// fed exactly once, so a dropped frame costs nothing. Once the script is
    /// through, scroll back into history, then restart from a fresh grid.
    fn play(&mut self, cx: &mut Context<Self>) {
        let elapsed = self.elapsed;
        if self.fed < self.script.len() {
            while self.fed < self.script.len() && self.script[self.fed].1 <= elapsed {
                let (bytes, _) = self.script[self.fed];
                self.emulator.feed(bytes);
                self.fed += 1;
            }
        } else {
            let end = self.script.last().map_or(0, |(_, at)| *at);
            if !self.scrolled && elapsed >= end + SCROLL_AT_MS {
                self.scrolled = true;
                self.emulator.scroll(5);
            }
            if elapsed >= end + RESTART_AT_MS {
                let cols = self.emulator.cols() as u16;
                let rows = self.emulator.rows() as u16;
                self.emulator = Emulator::new(cols, rows);
                self.fed = 0;
                self.scrolled = false;
                self.elapsed = 0;
            }
        }
        cx.notify();
    }

    /// The header strip: a live dot, the OSC title the script set, and what
    /// this page actually is.
    fn header(theme: &Theme, title: Option<&str>) -> gpui::Div {
        div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(8.0))
            .px(px(12.0))
            .h(px(36.0))
            .rounded_t(px(12.0))
            .bg(terminal_panel_bg(theme))
            .border_1()
            .border_b_0()
            .border_color(hairline(0.08))
            .child(div().size(px(8.0)).rounded_full().bg(theme.success))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .font_family(theme.font_mono.clone())
                    .text_size(px(12.0))
                    .text_color(theme.text)
                    .child(SharedString::from(title.unwrap_or("bezel"))),
            )
            .child(
                div()
                    .font_family(theme.font_mono.clone())
                    .text_size(px(11.0))
                    .text_color(theme.text_faint)
                    .child("scripted bytes — the PTY is the host's job"),
            )
    }
}

impl Render for Terminal {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // A backgrounded page stops rendering, so it needs a nudge to pick the
        // script back up when the window returns.
        if self.activation.is_none() {
            self.activation = Some(cx.observe_window_activation(window, |_, _, cx| cx.notify()));
        }
        if window.is_window_active() {
            self.schedule_tick(cx);
        } else {
            self.tick = None;
        }

        let theme = Theme::of(cx).clone();
        let this = cx.entity();
        let grid = TerminalElement::new(
            move |geometry, cx| {
                this.update(cx, |this, _| {
                    if this.emulator.cols() != geometry.cols as usize
                        || this.emulator.rows() != geometry.rows as usize
                    {
                        this.emulator.resize(geometry.cols, geometry.rows);
                    }
                    Some(GridSnapshot {
                        lines: this.emulator.lines(),
                        cursor: this.emulator.cursor(),
                    })
                })
            },
            true,
        );

        div().size_full().flex().justify_center().child(
            div()
                .w_full()
                .max_w(px(900.0))
                .px(px(24.0))
                .py(px(32.0))
                .flex()
                .flex_col()
                .child(Self::header(&theme, self.emulator.title()))
                .child(
                    div()
                        .flex_1()
                        .overflow_hidden()
                        .rounded_b(px(12.0))
                        .bg(terminal_panel_bg(&theme))
                        .border_1()
                        .border_t_0()
                        .border_color(hairline(0.08))
                        .child(grid),
                ),
        )
    }
}
