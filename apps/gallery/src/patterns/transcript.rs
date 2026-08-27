//! The transcript — a conversation as a scrollback, and the capstone of the
//! agent port: everything else built for it appears here at once.
//!
//! `scroll::follow` pins it to the newest line, `widgets::Takeover` runs each
//! turn's Work zone, `widgets::step_row` draws the tool calls, and
//! `markdown::markdown` renders the answers — which is the whole reason
//! this page could not be honest until that crate existed. Copy this file.
//!
//! **`Transcript.svelte` is 943 lines and produced no library code**, which was
//! the prediction and is now the measurement. Its three reducers are all std:
//!
//! - Turns — a question and the answer it drew — are `chunk_by`: start a chunk
//!   at every question.
//! - The zone split is `rposition`. **The answer is the prose after the last
//!   tool call; everything before it is interim** — that one sentence is the
//!   entire rule, and it is what stops a model's thinking-out-loud from being
//!   presented as its reply.
//! - A run of adjacent tool calls is `chunk_by` again, and the `Verb · N` fold
//!   inside it is the same `chunk_by` the Tool calls page uses.
//!
//! What is left of the 943 is Tauri `invoke`/`listen`, project lookups, sticky
//! scroll measurement and cloud-error parsing — an app's job, all of it.

use gpui::{AnyElement, Context, Render, SharedString, Window, div, prelude::*, px};
use motion::Painter;
use theme::Theme;
use ui::{
    icons, popover,
    scroll::{self, FollowState, ScrollbarState},
    widgets,
    widgets::{Layout, Status},
};

/// One thing that happened, in the order it happened. A page-local shape, and
/// deliberately not a library type: `step_row` takes strings, so bezel never
/// learns what a tool call is.
enum Beat {
    /// A question. Starts a turn, and carries the day it was asked on.
    Ask {
        text: &'static str,
        day: &'static str,
    },
    Thinking(&'static str),
    Tool {
        icon: &'static str,
        verb: &'static str,
        detail: &'static str,
        ms: u32,
        failed: bool,
    },
    /// Prose. Whether it reads as interim work or as the reply is not stored —
    /// it is decided by where the last tool call is. See [`split`].
    Text(&'static str),
}

impl Beat {
    fn is_ask(&self) -> bool {
        matches!(self, Beat::Ask { .. })
    }

    fn is_tool(&self) -> bool {
        matches!(self, Beat::Tool { .. })
    }
}

/// Two days of one conversation. Markdown in the answers, because that is what
/// an agent actually replies with and what this page exists to show.
const BEATS: &[Beat] = &[
    Beat::Ask {
        text: "Does bezel need a bezel-agent crate?",
        day: "Yesterday",
    },
    Beat::Thinking("The test is whether a non-agent app would want each piece."),
    Beat::Tool {
        icon: icons::BOOK,
        verb: "Read",
        detail: "ARCHITECTURE.md",
        ms: 2,
        failed: false,
    },
    Beat::Text("Checking what has actually been extracted so far."),
    Beat::Tool {
        icon: icons::BOOK,
        verb: "Read",
        detail: "todos/agent.md",
        ms: 3,
        failed: false,
    },
    Beat::Tool {
        icon: icons::BOOK,
        verb: "Read",
        detail: "crates/ui/src/widgets.rs",
        ms: 4,
        failed: false,
    },
    Beat::Tool {
        icon: icons::CPU,
        verb: "Recall",
        detail: "the control bar precedent",
        ms: 18,
        failed: false,
    },
    Beat::Text(
        "**No.** Every piece ported so far passes the test, so `ui` is holding \
         them:\n\n\
         - `scroll::follow` — a terminal wants this, and a build log wants it\n\
         - `widgets::Takeover` — a detail pane following a selection wants it\n\
         - `widgets::step_row` — a CI step is an operation with an outcome\n\n\
         The crate earns itself when a residue accumulates that a non-agent app \
         would *not* want. The thing to watch is a `ToolCall` type.",
    ),
    Beat::Ask {
        text: "What did the tool group cost?",
        day: "Today",
    },
    Beat::Tool {
        icon: icons::MAGNIFER,
        verb: "Search",
        detail: "chunk_by",
        ms: 9,
        failed: false,
    },
    Beat::Tool {
        icon: icons::TERMINAL,
        verb: "Run",
        detail: "cargo test -p ui",
        ms: 1_412,
        failed: false,
    },
    Beat::Tool {
        icon: icons::BOOK,
        verb: "Read",
        detail: "crates/agent/src/lib.rs",
        ms: 1,
        failed: true,
    },
    Beat::Text(
        "Nothing. `ToolGroup.svelte` is 54 lines of Svelte and zero lines of \
         bezel:\n\n\
         ```rust\n\
         calls.chunk_by(|a, b| a.verb == b.verb)\n\
         ```\n\n\
         The container around it is `rounded + border_1 + overflow_hidden`, \
         three lines at the call site.",
    ),
];

/// Where one turn's beats sit in [`BEATS`], and the index that splits them.
struct Turn {
    range: std::ops::Range<usize>,
    /// One past the last tool call, or the start when there was none: prose at
    /// or after this is the answer, prose before it is interim work.
    answer_from: usize,
}

/// Split the flat list into turns, and each turn into its two zones.
///
/// Both halves are std — `chunk_by` starts a chunk at every question,
/// `rposition` finds the last tool. The whole of `Transcript.svelte`'s
/// `splitTurn` is that second call.
fn split() -> Vec<Turn> {
    let mut turns = Vec::new();
    let mut start = 0;
    for chunk in BEATS.chunk_by(|_, b| !b.is_ask()) {
        let last_tool = chunk.iter().rposition(Beat::is_tool);
        turns.push(Turn {
            range: start..start + chunk.len(),
            answer_from: last_tool.map_or(0, |index| start + index + 1),
        });
        start += chunk.len();
    }
    turns
}

pub struct Transcript {
    /// Which turns have their Work zone open. Keyed by the turn's first beat —
    /// stable while `BEATS` is, and it is a `const`.
    work: std::collections::HashMap<usize, widgets::Takeover>,
    /// Tool calls showing their output, keyed by index into [`BEATS`].
    open_output: std::collections::HashSet<usize>,
    scroll: gpui::ScrollHandle,
    follow: FollowState,
    bar: ScrollbarState,
}

impl Transcript {
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            work: std::collections::HashMap::new(),
            open_output: std::collections::HashSet::new(),
            scroll: gpui::ScrollHandle::new(),
            follow: FollowState::new(),
            bar: ScrollbarState::new(Painter::of(cx)),
        }
    }
}

impl Transcript {
    /// The question, as its own surface. Right-aligned and capped so a long one
    /// wraps into a block rather than spanning the column.
    fn ask(theme: &Theme, text: &'static str) -> gpui::Div {
        div()
            .self_end()
            .max_w(px(440.0))
            .px(px(14.0))
            .py(px(9.0))
            .rounded(px(Theme::surface_radius()))
            .bg(theme.surface_raised)
            .text_size(px(13.5))
            .text_color(theme.text)
            .child(text)
    }

    /// The Work zone's header: how much happened, and a chevron to see it.
    ///
    /// `Takeover` runs it, with `auto = false` because every turn here is
    /// finished — in a live transcript that argument is the streaming flag, and
    /// the zone opens itself while the turn is running.
    fn work_header(
        &self,
        theme: &Theme,
        turn: usize,
        steps: usize,
        cx: &mut Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        let open = self.work.get(&turn).copied().unwrap_or_default().get(false);
        div()
            .id(SharedString::from(format!("work-{turn}")))
            .self_start()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(6.0))
            .px(px(4.0))
            .py(px(5.0))
            .rounded(px(Theme::control_radius()))
            .cursor_pointer()
            .hover(|s| s.bg(theme::ink(0.03)))
            .on_click(cx.listener(move |view: &mut Self, _, _, cx| {
                view.work.entry(turn).or_default().toggle(false);
                cx.notify();
            }))
            .child(theme.disclosure(open))
            .child(
                div()
                    .text_size(px(12.5))
                    .text_color(theme.text_muted)
                    .child(SharedString::from(format!("Worked · {steps} steps"))),
            )
    }

    /// One tool call, and its outcome. The same `step_row` the Tool calls page
    /// uses; a transcript is where it was going all along.
    fn tool(&self, theme: &Theme, index: usize, first: bool, cx: &mut Context<Self>) -> gpui::Div {
        let Beat::Tool {
            icon,
            verb,
            detail,
            ms,
            failed,
        } = &BEATS[index]
        else {
            return div();
        };
        let open = self.open_output.contains(&index);
        let output = failed.then_some("error: no such file or directory (os error 2)");
        div()
            .when(!first, |el| el.border_t_1().border_color(theme.border))
            .child(
                theme
                    .step_row(
                        icon,
                        *verb,
                        Some(SharedString::from(*detail)),
                        Some(SharedString::from(if *ms < 1000 {
                            format!("{ms}ms")
                        } else {
                            format!("{:.1}s", *ms as f32 / 1000.0)
                        })),
                        *failed,
                        output.map(|_| open),
                    )
                    .hover(widgets::step_row_hover)
                    .id(SharedString::from(format!("beat-{index}")))
                    .on_click(cx.listener(move |view: &mut Self, _, _, cx| {
                        if !view.open_output.insert(index) {
                            view.open_output.remove(&index);
                        }
                        cx.notify();
                    })),
            )
            .when_some(output.filter(|_| open), |el, output| {
                el.child(theme.step_output(SharedString::from(format!("beat-out-{index}")), output))
            })
    }

    /// A turn's interim half: thinking, prose, and runs of tool calls boxed
    /// together. The run boundary is `chunk_by` on "is this a tool", so a
    /// sentence between two calls breaks the box exactly where it should.
    fn work(&self, theme: &Theme, turn: &Turn, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let interim = &BEATS[turn.range.start..turn.answer_from];
        let mut out = Vec::new();
        let mut index = turn.range.start;
        for run in interim.chunk_by(|a, b| a.is_tool() == b.is_tool()) {
            if run[0].is_tool() {
                let start = index;
                out.push(
                    div()
                        .rounded(px(Theme::panel_radius()))
                        .border_1()
                        .border_color(theme.border)
                        .overflow_hidden()
                        .children(
                            (start..start + run.len())
                                .map(|i| self.tool(theme, i, i == start, cx).into_any_element()),
                        )
                        .into_any_element(),
                );
            } else {
                out.extend(run.iter().map(|beat| {
                    match beat {
                        Beat::Thinking(text) => div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(px(6.0))
                            .text_size(px(12.5))
                            .text_color(theme.text_muted.opacity(0.7))
                            .child(
                                icons::icon(icons::CPU)
                                    .size(px(12.0))
                                    .text_color(theme.text_faint),
                            )
                            .child(*text)
                            .into_any_element(),
                        Beat::Text(text) => div()
                            .text_size(px(12.5))
                            .text_color(theme.text_muted)
                            .child(*text)
                            .into_any_element(),
                        _ => div().into_any_element(),
                    }
                }));
            }
            index += run.len();
        }
        out
    }
}

impl Render for Transcript {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::of(cx).clone();
        let turns = split();
        let mut day = "";
        let mut rows: Vec<AnyElement> = Vec::new();

        for turn in &turns {
            let first = turn.range.start;
            let Beat::Ask { text, day: on } = &BEATS[first] else {
                continue;
            };
            // The heading appears where the day *changes*, which is the whole
            // of `sameDay` — and the label is the app's word for it, since
            // bezel carries no clock and no date vocabulary.
            if *on != day {
                day = on;
                rows.push(
                    div()
                        .py(px(8.0))
                        .text_size(px(11.0))
                        .font_weight(gpui::FontWeight::MEDIUM)
                        .text_color(theme.text_faint)
                        .child(SharedString::from(popover::tracked_upper(on)))
                        .into_any_element(),
                );
            }

            let steps = BEATS[turn.range.start..turn.answer_from]
                .iter()
                .filter(|beat| beat.is_tool())
                .count();
            let open = self
                .work
                .get(&first)
                .copied()
                .unwrap_or_default()
                .get(false);

            let mut zone = div()
                .flex()
                .flex_col()
                .gap(px(10.0))
                .child(Self::ask(&theme, text));
            if steps > 0 {
                zone = zone.child(self.work_header(&theme, first, steps, cx));
                if open {
                    zone = zone.child(
                        div()
                            .ml(px(10.0))
                            .pl(px(12.0))
                            .border_l_1()
                            .border_color(theme.border)
                            .flex()
                            .flex_col()
                            .gap(px(8.0))
                            .children(self.work(&theme, turn, cx)),
                    );
                }
            }
            // The answer zone, rendered as markdown — the reason this page
            // waited for `markdown` rather than shipping with a
            // paragraph and calling it done.
            for beat in &BEATS[turn.answer_from..turn.range.end] {
                if let Beat::Text(source) = beat {
                    zone = zone.child(markdown::markdown(source, window, cx));
                }
            }
            rows.push(zone.pb(px(28.0)).into_any_element());
        }

        div().size_full().flex().justify_center().child(
            div()
                .relative()
                .w_full()
                .max_w(px(700.0))
                .child(
                    div()
                        .id("transcript")
                        .size_full()
                        .overflow_y_scroll()
                        .track_scroll(&self.scroll)
                        .child(
                            div()
                                .px(px(24.0))
                                .py(px(28.0))
                                .flex()
                                .flex_col()
                                .children(rows),
                        ),
                )
                // The same pair the Activity page uses, over a whole
                // conversation this time.
                .child(scroll::follow(&self.scroll, &self.follow))
                .child(scroll::scrollbar("transcript-bar", &self.scroll, &self.bar)),
        )
    }
}
