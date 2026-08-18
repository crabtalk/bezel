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
use bezel_ui::{
    icons,
    input::{Shape, TextField},
    loaders, popover,
    scroll::{self, FollowState, ScrollbarState},
    widgets,
    widgets::{ButtonStyle, Buttons, Layout, Status},
};
use gpui::{
    AnyElement, Context, Entity, Render, SharedString, Window, div, linear_color_stop,
    linear_gradient, prelude::*, px,
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
            // The orb cluster, at the slot the chevron leaves behind: while a
            // model is thinking, this is what bezel shows.
            loaders::orb(loaders::Orb::Cluster, "thinking", 14.0, theme, view, cx)
                .into_any_element()
        } else {
            theme.disclosure(open).into_any_element()
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
                            .child(theme.button(
                                "Ask again",
                                ButtonStyle::Ghost,
                                Some("agent-ask".into()),
                            )),
                    ),
            )
    }
}

// ---------------------------------------------------------------------------
// Tool calls — `ToolCard.svelte` + `ToolGroup.svelte`
// ---------------------------------------------------------------------------

/// One call in the run below. This is the shape bezel deliberately does *not*
/// have a type for: `widgets::step_row` takes the four strings and a flag, so
/// the library never learns what a tool is. Yours will look nothing like this
/// one, and that is the point.
struct Call {
    icon: &'static str,
    verb: &'static str,
    detail: &'static str,
    ms: u32,
    failed: bool,
    /// `None` prints no chevron — a call that returned nothing has nothing to
    /// open.
    output: Option<&'static str>,
}

/// A turn's worth of calls, in the order they ran. The three consecutive
/// `Read`s are the case the grouping exists for.
const CALLS: [Call; 12] = [
    Call {
        icon: icons::COMPASS,
        verb: "Discover",
        detail: "sources for \"bezel-agent\"",
        ms: 40,
        failed: false,
        output: Some("ARCHITECTURE.md\nTODO.md\ntodos/agent.md"),
    },
    Call {
        icon: icons::MAGNIFER,
        verb: "Search",
        detail: "fn at_bottom",
        ms: 12,
        failed: false,
        output: Some("crates/ui/src/scroll.rs:236\ncrates/ui/src/scroll.rs:411"),
    },
    Call {
        icon: icons::BOOK,
        verb: "Read",
        detail: "crates/ui/src/scroll.rs",
        ms: 3,
        failed: false,
        output: None,
    },
    Call {
        icon: icons::BOOK,
        verb: "Read",
        detail: "crates/ui/src/widgets.rs",
        ms: 2,
        failed: false,
        output: None,
    },
    Call {
        icon: icons::BOOK,
        verb: "Read",
        detail: "ARCHITECTURE.md",
        ms: 2,
        failed: false,
        output: Some("extracted from working application code, never invented ahead of need"),
    },
    Call {
        icon: icons::TERMINAL,
        verb: "Run",
        detail: "cargo test -p bezel-ui",
        ms: 1_412,
        failed: false,
        output: Some("running 87 tests\n\ntest result: ok. 87 passed; 0 failed"),
    },
    Call {
        icon: icons::CPU,
        verb: "Recall",
        detail: "what bezel-agent was for",
        ms: 18,
        failed: false,
        output: Some("a crate agreed in principle and deliberately not created"),
    },
    Call {
        icon: icons::DOWNLOAD,
        verb: "Fetch",
        detail: "aicss.dev",
        ms: 210,
        failed: false,
        output: Some("14 blocks, none of them measured against a running app"),
    },
    Call {
        icon: icons::LINK,
        verb: "Link",
        detail: "todos/agent.md → TODO.md",
        ms: 5,
        failed: false,
        output: None,
    },
    Call {
        icon: icons::GIT_BRANCH,
        verb: "Diff",
        detail: "crates/ui",
        ms: 31,
        failed: false,
        output: None,
    },
    Call {
        icon: icons::BOOK,
        verb: "Read",
        detail: "crates/agent/src/lib.rs",
        ms: 1,
        failed: true,
        output: Some("error: no such file or directory (os error 2)"),
    },
    Call {
        icon: icons::PEN,
        verb: "Edit",
        detail: "todos/agent.md",
        ms: 7,
        failed: false,
        output: None,
    },
];

/// `1412ms` under a second, `1.4s` over it — a figure you read at a glance
/// rather than count digits in.
fn took(ms: u32) -> String {
    if ms < 1000 {
        format!("{ms}ms")
    } else {
        format!("{:.1}s", ms as f32 / 1000.0)
    }
}

/// A run of tool calls, grouped the way `ToolGroup.svelte` groups them.
///
/// **The grouping is `slice::chunk_by`** — consecutive calls of the same verb,
/// straight out of std, and bezel wrote nothing for it. That is the whole
/// finding of this page: what looked like a component ("tool group") is one
/// std call plus a container that already existed.
///
/// The two variants are here as well, and they are not a parameter anywhere in
/// the library: a lone call is a bordered card, a grouped one is a bare row and
/// the group's box owns the border and the hairlines between them.
pub struct ToolCalls {
    /// Groups showing their calls, keyed by the index of the first call in the
    /// run — stable as long as `CALLS` is, and it is a `const`.
    open_groups: std::collections::HashSet<usize>,
    /// Calls showing their output, keyed by index into `CALLS`.
    open_output: std::collections::HashSet<usize>,
}

/// The page opens with the run of `Read`s already open. Every other row is
/// collapsed, which is `ToolGroup.svelte`'s own default — this one is unfolded
/// because the nesting is the thing the page is here to show, and a visitor
/// should not have to guess that a row opens before they see it.
impl Default for ToolCalls {
    fn default() -> Self {
        // The first run of more than one call — *found*, not written down: an
        // index here would go stale the next time a call is added above it,
        // and the page would open on nothing with no error to show for it.
        let mut start = 0;
        let mut open_groups = std::collections::HashSet::new();
        for run in CALLS.chunk_by(|a, b| a.verb == b.verb) {
            if run.len() > 1 {
                open_groups.insert(start);
                break;
            }
            start += run.len();
        }
        Self {
            open_groups,
            open_output: Default::default(),
        }
    }
}

impl ToolCalls {
    /// One call as a row, plus its output when open. `first` draws the hairline
    /// above it, so a group's box needs no divider of its own.
    fn call(&self, theme: &Theme, index: usize, first: bool, cx: &mut Context<Self>) -> gpui::Div {
        let call = &CALLS[index];
        let open = self.open_output.contains(&index);
        div()
            .when(!first, |el| el.border_t_1().border_color(theme.border))
            .child(
                theme
                    .step_row(
                        call.icon,
                        call.verb,
                        Some(SharedString::from(call.detail)),
                        Some(SharedString::from(took(call.ms))),
                        call.failed,
                        call.output.map(|_| open),
                    )
                    .hover(widgets::step_row_hover)
                    .id(SharedString::from(format!("call-{index}")))
                    .on_click(cx.listener(move |view, _, _, cx| {
                        if !view.open_output.insert(index) {
                            view.open_output.remove(&index);
                        }
                        cx.notify();
                    })),
            )
            .when_some(call.output.filter(|_| open), |el, output| {
                el.child(theme.step_output(SharedString::from(format!("call-out-{index}")), output))
            })
    }

    /// The box a card and a group share: rounded, bordered, clipping whatever
    /// it holds. Three lines, which is why there is no `variant` parameter in
    /// `ui` for it.
    fn box_(theme: &Theme) -> gpui::Div {
        div()
            .rounded(px(Theme::PANEL_RADIUS))
            .border_1()
            .border_color(theme.border)
            .overflow_hidden()
    }
}

impl Render for ToolCalls {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::of(cx).clone();

        // The grouping, whole. `chunk_by` hands back consecutive runs; the
        // offset is what turns a run back into indices into `CALLS`.
        let mut offset = 0;
        let mut groups = Vec::new();
        for run in CALLS.chunk_by(|a, b| a.verb == b.verb) {
            groups.push((offset, run.len()));
            offset += run.len();
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
                    .gap(px(8.0))
                    .children(groups.into_iter().map(|(start, len)| {
                        if len == 1 {
                            return Self::box_(&theme)
                                .child(self.call(&theme, start, true, cx))
                                .into_any_element();
                        }
                        let open = self.open_groups.contains(&start);
                        let failed = CALLS[start..start + len].iter().any(|call| call.failed);
                        Self::box_(&theme)
                            .child(
                                theme
                                    .step_row(
                                        CALLS[start].icon,
                                        CALLS[start].verb,
                                        Some(SharedString::from(format!("· {len}"))),
                                        None,
                                        failed,
                                        Some(open),
                                    )
                                    .hover(widgets::step_row_hover)
                                    .id(SharedString::from(format!("group-{start}")))
                                    .on_click(cx.listener(move |view, _, _, cx| {
                                        if !view.open_groups.insert(start) {
                                            view.open_groups.remove(&start);
                                        }
                                        cx.notify();
                                    })),
                            )
                            .when(open, |group| {
                                group.child(
                                    div()
                                        .border_t_1()
                                        .border_color(theme.border)
                                        .pl(px(16.0))
                                        .children((start..start + len).map(|index| {
                                            self.call(&theme, index, index == start, cx)
                                                .into_any_element()
                                        })),
                                )
                            })
                            .into_any_element()
                    })),
            )
    }
}

// ---------------------------------------------------------------------------
// Composer — `Composer.svelte` + `PromptEditor.svelte`
// ---------------------------------------------------------------------------

gpui::actions!(
    bezel_gallery_composer,
    [
        Send,
        MentionNext,
        MentionPrevious,
        MentionConfirm,
        MentionDismiss
    ]
);

/// The context this page's field claims on top of `TextField`/`TextArea`, so
/// `enter` sends **here** and stays a newline in every other multi-line field
/// in the gallery. See [`bezel_ui::input::TextField::with_key_context`] for why
/// a container around the field cannot do this.
const COMPOSER_CONTEXT: &str = "GalleryComposer";

/// Bind the composer's keys. Called once at startup beside `input::init`,
/// because a pattern is an app: the bindings belong to whoever mounts the page,
/// not to `ui`.
pub fn init(cx: &mut gpui::App) {
    let ctx = Some(COMPOSER_CONTEXT);
    cx.bind_keys([
        gpui::KeyBinding::new("enter", Send, ctx),
        // Bound explicitly: the field's own `enter` is what usually inserts a
        // newline, and this page has just taken it.
        gpui::KeyBinding::new("shift-enter", bezel_ui::input::InsertNewline, ctx),
        gpui::KeyBinding::new("down", MentionNext, ctx),
        gpui::KeyBinding::new("up", MentionPrevious, ctx),
        gpui::KeyBinding::new("escape", MentionDismiss, ctx),
    ]);
}

/// What `#` offers. Desktop searches its note store; bezel takes a list of
/// strings, which is the whole difference between a library and an app.
const MENTIONS: [&str; 7] = [
    "ARCHITECTURE.md",
    "TODO.md",
    "crates/ui/src/input.rs",
    "crates/ui/src/popover.rs",
    "crates/ui/src/scroll.rs",
    "crates/ui/src/widgets.rs",
    "todos/agent.md",
];

/// The composer: a growing field in a frosted card, a send button that knows
/// when there is nothing to send, and the `#` picker.
///
/// `Shape::Grow { min, max }` **is** `PromptEditor`'s `rows`/`maxRows`, and
/// `control_bar(Shape::Rounded)` is its pill — so the only thing this page had
/// to invent is the picker, and the picker is `popover::Filter` (the combobox's
/// own state) mounted at a caret instead of under a trigger.
pub struct Composer {
    field: Entity<TextField>,
    /// Byte offset of the `#` being typed, or `None` when no mention is open.
    /// Derived from the text on every change rather than stored as a flag: a
    /// backspace over the `#` has to close the picker, and a flag would have to
    /// be told.
    mention: Option<usize>,
    filter: popover::Filter,
    /// What has been sent, newest last — the page needs somewhere for a message
    /// to go, and a transcript is the next pattern rather than this one.
    sent: Vec<SharedString>,
    focus_handle: gpui::FocusHandle,
}

impl Composer {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let field = cx.new(|cx| {
            TextField::new(cx)
                .with_shape(Shape::Grow { min: 3, max: 12 })
                .with_key_context(COMPOSER_CONTEXT)
                .with_placeholder("Ask anything, or # to attach a file")
        });
        cx.observe(&field, |composer: &mut Self, _, cx| composer.reread(cx))
            .detach();
        Self {
            field,
            mention: None,
            filter: popover::Filter::new(MENTIONS.iter().map(|&m| m.into()).collect()),
            sent: Vec::new(),
            focus_handle: cx.focus_handle(),
        }
    }

    /// The whole picker trigger, and it is a *read* of the text rather than a
    /// key handler: the `#` nearest behind the caret, if nothing since it has
    /// been whitespace. That way typing, pasting, arrowing back into a word and
    /// deleting the `#` all agree without any of them being special-cased.
    fn reread(&mut self, cx: &mut Context<Self>) {
        let content = self.field.read(cx).content().clone();
        let caret = self.field.read(cx).cursor().min(content.len());
        self.mention = content[..caret]
            .rfind('#')
            .filter(|&hash| !content[hash + 1..caret].contains(char::is_whitespace));
        if let Some(hash) = self.mention {
            self.filter.refilter(&content[hash + 1..caret]);
        }
        cx.notify();
    }

    /// Replace `#query` with the picked path and close.
    fn accept(&mut self, item: usize, cx: &mut Context<Self>) {
        let Some(hash) = self.mention else { return };
        let content = self.field.read(cx).content().clone();
        let caret = self.field.read(cx).cursor().min(content.len());
        let picked = format!("{}{} ", &content[..hash], self.filter.items()[item]);
        let rest = content[caret..].to_string();
        self.field
            .update(cx, |field, cx| field.set_content(picked + &rest, cx));
        self.mention = None;
        cx.notify();
    }

    fn send(&mut self, _: &Send, _: &mut Window, cx: &mut Context<Self>) {
        // `enter` is one key doing two jobs: while the picker is up it takes
        // the highlighted row, exactly as the combobox's does.
        if self.mention.is_some()
            && let Some(item) = self.filter.active_item()
        {
            self.accept(item, cx);
            return;
        }
        let content = self.field.read(cx).content().clone();
        if content.trim().is_empty() {
            return;
        }
        self.sent.push(content);
        self.field.update(cx, |field, cx| field.clear(cx));
        self.mention = None;
        cx.notify();
    }

    fn mention_next(&mut self, _: &MentionNext, _: &mut Window, cx: &mut Context<Self>) {
        self.filter.step(1);
        cx.notify();
    }

    fn mention_previous(&mut self, _: &MentionPrevious, _: &mut Window, cx: &mut Context<Self>) {
        self.filter.step(-1);
        cx.notify();
    }

    fn mention_dismiss(&mut self, _: &MentionDismiss, _: &mut Window, cx: &mut Context<Self>) {
        self.mention = None;
        cx.notify();
    }

    /// Send, as a 24px disc — and inert until there is something to send.
    ///
    /// `Composer.svelte` dims the whole button to `opacity-30`, which on a
    /// frosted card reads as a grey blob with a grey arrow inside it. bezel
    /// already has a word for a control that is present but not pressable, and
    /// it is [`bezel_ui::pagination::step`]: the shape keeps its place, the
    /// glyph goes faint, and the pointer and hover simply are not there. Same
    /// rule here — the disc quietens to the hover wash instead of fading, so
    /// the arrow stays legible and nothing invites a press.
    fn send_button(
        &self,
        theme: &Theme,
        ready: bool,
        cx: &mut Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        let disc = div()
            .size(px(24.0))
            .rounded_full()
            .flex()
            .items_center()
            .justify_center();
        let disc = if ready {
            disc.bg(theme.solid)
                .cursor_pointer()
                .hover(|s| s.opacity(0.9))
                .child(
                    icons::icon(icons::ARROW_UP)
                        .size(px(14.0))
                        .text_color(theme.on_solid),
                )
        } else {
            disc.bg(bezel_theme::ink(0.06)).child(
                icons::icon(icons::ARROW_UP)
                    .size(px(14.0))
                    .text_color(theme.text_faint),
            )
        };
        div()
            .id("composer-send")
            .on_click(cx.listener(|composer, _, window, cx| composer.send(&Send, window, cx)))
            .child(disc)
    }

    /// The picker, anchored at the `#` itself rather than under the card —
    /// `TextField::offset_bounds` is the same measurement the IME candidate
    /// panel anchors to, so it follows the caret down as the box grows.
    fn picker(&self, theme: &Theme, window: &Window, cx: &mut Context<Self>) -> Option<AnyElement> {
        let hash = self.mention?;
        let anchor = self.field.read(cx).offset_bounds(hash, window)?;
        let rows: Vec<AnyElement> = self
            .filter
            .filtered()
            .iter()
            .enumerate()
            .map(|(position, &item)| {
                popover::menu_row(
                    theme,
                    Some(position) == self.filter.active(),
                    format!("mention-{item}"),
                )
                .id(SharedString::from(format!("mention-{item}")))
                .on_click(cx.listener(move |composer, _, _, cx| composer.accept(item, cx)))
                .child(self.filter.items()[item].clone())
                .into_any_element()
            })
            .collect();
        if rows.is_empty() {
            return None;
        }
        Some(popover::menu_at(
            "composer-mentions",
            // The row's bottom-left: the menu hangs under the `#`, the way the
            // candidate panel hangs under composing text.
            gpui::point(anchor.left(), anchor.bottom() + px(4.0)),
            popover::popover_card(theme)
                .w(px(280.0))
                .child(div().flex().flex_col().children(rows))
                .into_any_element(),
            None,
        ))
    }
}

impl gpui::Focusable for Composer {
    fn focus_handle(&self, _: &gpui::App) -> gpui::FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for Composer {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::of(cx).clone();
        let ready = !self.field.read(cx).content().trim().is_empty();
        let picker = self.picker(&theme, window, cx);
        let sent: Vec<AnyElement> = self
            .sent
            .iter()
            .map(|message| {
                div()
                    .self_end()
                    .max_w(px(440.0))
                    .px(px(14.0))
                    .py(px(9.0))
                    .rounded(px(Theme::SURFACE_RADIUS))
                    .bg(theme.surface_raised)
                    .text_size(px(13.5))
                    .text_color(theme.text)
                    .child(message.clone())
                    .into_any_element()
            })
            .collect();

        div()
            .key_context("GalleryComposerPage")
            .track_focus(&self.focus_handle)
            .on_action(cx.listener(Self::send))
            .on_action(cx.listener(Self::mention_next))
            .on_action(cx.listener(Self::mention_previous))
            .on_action(cx.listener(Self::mention_dismiss))
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
                    .gap(px(12.0))
                    .children(sent)
                    // The card: `Card variant="input"` — the field, then a row
                    // of controls under it, both on one frosted surface.
                    .child(
                        div()
                            .rounded(px(Theme::SURFACE_RADIUS))
                            .border_1()
                            .border_color(theme.border)
                            .bg(theme.card_glass_bg())
                            .px(px(4.0))
                            .pt(px(4.0))
                            .pb(px(6.0))
                            .flex()
                            .flex_col()
                            .gap(px(4.0))
                            .child(self.field.clone())
                            .child(
                                div()
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .justify_between()
                                    .px(px(6.0))
                                    .child(
                                        div()
                                            .text_size(px(11.5))
                                            .font_family(theme.font_mono.clone())
                                            .text_color(theme.text_faint)
                                            .child(if self.mention.is_some() {
                                                "↑↓ pick · enter attach · esc close"
                                            } else {
                                                "enter send · shift-enter newline"
                                            }),
                                    )
                                    .child(self.send_button(&theme, ready, cx)),
                            ),
                    )
                    .children(picker),
            )
    }
}
