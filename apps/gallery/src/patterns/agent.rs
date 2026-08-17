//! The agent activity pattern — a model working in public, which is the one
//! screen an agent app cannot borrow from anything else.
//!
//! It is `../desktop`'s `LiveActivity.svelte` and `Thought.svelte` composed, and
//! composing them is what showed both pieces were library code:
//! [`bezel_ui::scroll::follow`] pins the reasoning box to its newest line while
//! the run writes into it, and [`bezel_ui::widgets::Takeover`] opens the section
//! while that is happening and hands it over the moment you press the header.
//! Everything else here is a `div`. Copy this file.
//!
//! **The answer is plain text on purpose.** Streaming markdown is
//! `bezel-markdown`'s job and that crate does not exist yet, so a paragraph is
//! the honest placeholder — this page claims the *activity* zone works, not the
//! answer zone. That is also why there is no transcript here: one exchange is
//! all the parts can carry today.
//!
//! Like the music player it is an entity, for the reason stated there: a screen
//! owns a screen's worth of state, and its host holds one field.

use std::time::{Duration, Instant};

use bezel_theme::Theme;
use bezel_ui::scroll::{self, FollowState, ScrollbarState};
use bezel_ui::{loaders, popover, widgets};
use gpui::{
    Context, Render, SharedString, Window, div, linear_color_stop, linear_gradient, prelude::*, px,
};

/// The question on the page. Invented, like the music player's album, and about
/// this repository because a reasoning trace has to be *about* something before
/// it reads as one.
const PROMPT: &str = "Why does bezel have no bezel-agent crate yet?";

/// The reasoning, one line per beat. A real one arrives as tokens; the unit
/// here is a line because that is what the box scrolls by.
const REASONING: [&str; 14] = [
    "Reading ARCHITECTURE.md.",
    "\"extracted from working application code, never invented ahead of need\"",
    "— so the question is what has actually been extracted.",
    "Reading TODO.md, the porting table.",
    "Two pieces have landed: scroll::follow and widgets::Takeover.",
    "Checking where each one went.",
    "Both are in crates/ui, not in a crate of their own.",
    "scroll::follow — a terminal wants this. A build log wants this.",
    "Takeover — a detail pane following a selection wants this.",
    "Neither fails \"would a non-agent app want it\".",
    "Looking for the residue a new crate would hold.",
    "There isn't any yet.",
    "The music player is the precedent: it set out to need bezel-media",
    "and produced a control bar, which was general, so it went to ui.",
];

/// What it says once it stops. Plain text — see the module note.
const ANSWER: &str = "Because nothing has failed the test yet. Both pieces \
    ported so far are general enough that a terminal or a build log would want \
    them, so they went to `ui` beside the components they compose with. The \
    crate earns itself when a residue accumulates that a non-agent app would \
    not want — and the thing to watch for is a `ToolCall` type, because that is \
    a data model, and bezel refuses those.";

/// How tall the reasoning box may get before it scrolls instead — Tailwind's
/// `max-h-40`, which is what `LiveActivity.svelte` caps it at.
const BOX_MAX: f32 = 160.0;
/// The fade along its top edge: that component's `1.25rem` mask.
const FADE: f32 = 20.0;

/// How long a line takes to arrive. A demo parameter, chosen like the album
/// title: fast enough that the whole run fits in one look, slow enough that you
/// can scroll up mid-run and be left there, which is the behaviour on trial.
const LINE: Duration = Duration::from_millis(260);

/// How long the whole run takes — and so what the collapsed header reports.
/// Derived rather than written down, because the two would drift.
fn duration() -> f32 {
    REASONING.len() as f32 * LINE.as_secs_f32()
}

pub struct Activity {
    /// When the run in flight started, or `None` for one that finished long
    /// ago. The page opens on `None` so you land on a whole answer and press to
    /// watch it happen, rather than on an empty box mid-thought.
    run: Option<Instant>,
    /// Whether the reasoning section is open. It follows the run until pressed
    /// — that rule *is* [`widgets::Takeover`], and this page exists to show it
    /// against a real one.
    thought: widgets::Takeover,
    scroll: gpui::ScrollHandle,
    follow: FollowState,
    bar: ScrollbarState,
}

impl Default for Activity {
    fn default() -> Self {
        Self {
            run: None,
            thought: widgets::Takeover::default(),
            scroll: gpui::ScrollHandle::new(),
            follow: FollowState::new(),
            bar: ScrollbarState::new(),
        }
    }
}

impl Activity {
    /// How many lines have arrived. Derived from the wall clock like the music
    /// player's position, so a dropped frame costs nothing and no per-frame
    /// counter drifts away from the truth.
    fn shown(&self) -> usize {
        match self.run {
            None => REASONING.len(),
            Some(started) => {
                let lines = (started.elapsed().as_secs_f32() / LINE.as_secs_f32()) as usize;
                lines.min(REASONING.len())
            }
        }
    }

    /// Still writing. Not a flag: the run is over exactly when the last line has
    /// arrived, so the two cannot disagree.
    fn running(&self) -> bool {
        self.shown() < REASONING.len()
    }

    /// Start again from the first line.
    ///
    /// The takeover resets with it, and that is the interesting line: a new run
    /// is a new section, so it goes back to following. In `../desktop` this
    /// falls out of the component being re-mounted; here it has to be said.
    fn ask(&mut self, cx: &mut Context<Self>) {
        self.run = Some(Instant::now());
        self.thought = widgets::Takeover::default();
        self.follow.follow();
        cx.notify();
    }

    /// The header: a spinner while it writes, a chevron and how long it took
    /// once it stops — and clickable either way, which is the whole point. The
    /// id and the click are on the row itself rather than on a wrapper, because
    /// a wrapper takes the clicks over a box narrower than what it paints.
    fn header(
        &self,
        theme: &Theme,
        open: bool,
        cx: &mut Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        let running = self.running();
        let view = cx.entity_id();
        // Built here rather than inside the row, because gpui reads an svg's
        // colour off that element's own style: a chevron tinted by its parent
        // paints nothing at all.
        let glyph = if running {
            loaders::mini_gradient_spinner("thinking", 2.5, view, cx).into_any_element()
        } else {
            widgets::disclosure(theme, open).into_any_element()
        };
        div()
            .id("thought-header")
            .self_start()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(6.0))
            .px(px(4.0))
            .py(px(5.0))
            .rounded(px(Theme::CONTROL_RADIUS))
            .cursor_pointer()
            .hover(|s| s.bg(bezel_theme::ink(0.03)))
            .on_click(cx.listener(move |view, _, _, cx| {
                let running = view.running();
                view.thought.toggle(running);
                cx.notify();
            }))
            .child(div().flex().w(px(14.0)).justify_center().child(glyph))
            .child(
                div()
                    .text_size(px(12.5))
                    .text_color(theme.text_muted)
                    .child(SharedString::from(if running {
                        "Thinking".to_string()
                    } else {
                        format!("Thought for {:.0}s", duration())
                    })),
            )
    }

    /// The reasoning box: the lines that have arrived, pinned to the newest one.
    ///
    /// A *cap* rather than a height, which is `LiveActivity.svelte`'s `max-h-40`
    /// — the box grows with the first few lines and only then starts scrolling,
    /// so a run that has just begun is not a mostly-empty well. The bar and the
    /// pin are laid over the same container, which owns its own
    /// `overflow_y_scroll`.
    ///
    /// The strip along the top is that component's mask (`transparent` to
    /// `black` over `1.25rem`) painted rather than masked, since gpui has no
    /// mask-image at the pinned rev: a line leaving the box fades into the page
    /// instead of being cut in half by an edge.
    fn reasoning(&self, theme: &Theme) -> gpui::Div {
        div()
            .ml(px(10.0))
            .pl(px(12.0))
            .border_l_1()
            .border_color(theme.border)
            .child(
                div()
                    .relative()
                    .max_h(px(BOX_MAX))
                    .child(
                        div()
                            .id("reasoning")
                            .max_h(px(BOX_MAX))
                            .overflow_y_scroll()
                            .track_scroll(&self.scroll)
                            .child(div().flex().flex_col().gap(px(4.0)).pr(px(14.0)).children(
                                REASONING.iter().take(self.shown()).map(|line| {
                                    div()
                                        .text_size(px(12.5))
                                        .text_color(theme.text_muted.opacity(0.7))
                                        .child(SharedString::from(*line))
                                }),
                            )),
                    )
                    .child(div().absolute().top_0().left_0().right_0().h(px(FADE)).bg(
                        linear_gradient(
                            180.0,
                            linear_color_stop(theme.bg, 0.0),
                            linear_color_stop(theme.bg.opacity(0.0), 1.0),
                        ),
                    ))
                    .child(scroll::follow(&self.scroll, &self.follow))
                    .child(scroll::scrollbar("reasoning-bar", &self.scroll, &self.bar)),
            )
    }
}

impl Render for Activity {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::of(cx).clone();
        let running = self.running();
        let open = self.thought.get(running);

        // Same rule as the music player's clock: the line count is derived at
        // paint time, so the only thing that makes it move is asking for the
        // next frame. A finished page costs nothing.
        if running {
            window.request_animation_frame();
        }

        div()
            .size_full()
            .flex()
            .justify_center()
            .overflow_hidden()
            .child(
                div()
                    .w_full()
                    .max_w(px(660.0))
                    .px(px(24.0))
                    .py(px(32.0))
                    .flex()
                    .flex_col()
                    .gap(px(16.0))
                    // The question, as the surface every agent app puts it on.
                    .child(
                        div()
                            .self_end()
                            .max_w(px(440.0))
                            .px(px(14.0))
                            .py(px(9.0))
                            .rounded(px(Theme::SURFACE_RADIUS))
                            .bg(theme.surface_raised)
                            .text_size(px(13.5))
                            .text_color(theme.text)
                            .child(PROMPT),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(4.0))
                            .child(self.header(&theme, open, cx))
                            .when(open, |zone| zone.child(self.reasoning(&theme))),
                    )
                    // The answer zone. It arrives whole because there is nothing
                    // yet that could stream it — see the module note.
                    .when(!running, |page| {
                        page.child(
                            div()
                                .text_size(px(13.5))
                                .text_color(theme.text)
                                .child(ANSWER),
                        )
                    })
                    .child(
                        div()
                            .id("ask")
                            .self_start()
                            .on_click(cx.listener(|view, _, _, cx| view.ask(cx)))
                            .child(popover::button(&theme, "Ask again", "agent-ask")),
                    ),
            )
    }
}
