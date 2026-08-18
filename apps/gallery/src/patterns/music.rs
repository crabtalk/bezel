//! The music player pattern — the Patterns tab's first entry, and the reason
//! [`bezel_ui::control_bar`] exists.
//!
//! Nothing here is library code and none of it needs to be. A player is a
//! sidebar, a table and a floating bar, and bezel already had two of the three;
//! composing it is what showed that the third was missing. Copy this file.
//!
//! The two pieces worth reading before you copy them are [`MusicPlayer::elapsed`]
//! (the clock) and [`MusicPlayer::scrub`] (why the thumb does not fight it).
//!
//! It is an entity rather than a handful of fields on the gallery, and that is
//! the rule for every pattern after it: a component demo owns a value or two
//! and can borrow its host's, but a screen owns a screen's worth of state. Its
//! host holds one field, and the next pattern costs one more.

use bezel_theme::Theme;
use bezel_ui::{
    control_bar::{self, BAR_HEIGHT, Shape},
    icons, popover, scroll,
    table::{self, Column, Width},
    widgets,
    widgets::{Controls, Scaffolding, SliderDrag},
};
use gpui::{
    AnyElement, Axis, Context, DragMoveEvent, MouseButton, MouseDownEvent, Render, SharedString,
    Window, div, linear_color_stop, linear_gradient, prelude::*, px,
};
use std::collections::HashSet;

/// The album on the page. Invented, and it has to be: quoting a real catalogue
/// in a component gallery is a licensing question rather than documentation.
const ALBUM: &str = "Parallel Lines";
const ARTIST: &str = "Static Field";

/// `(title, featured artist, seconds)`.
const TRACKS: [(&str, &str, u32); 9] = [
    ("Long Exposure", "Static Field", 221),
    ("Grain", "Static Field", 198),
    ("Nightbus", "Static Field, Mora", 264),
    ("Held Note", "Static Field", 175),
    ("Copper Wire", "Static Field", 302),
    ("Salt Flats", "Static Field", 246),
    ("Low Ceiling", "Static Field, Ilse", 188),
    ("Parallel Lines", "Static Field", 331),
    ("Return Path", "Static Field", 209),
];

/// The library rail. `(icon, label)` — ordinary app chrome, and the shape every
/// player has.
const LIBRARY: [(&str, &str); 4] = [
    (icons::HEART, "Liked songs"),
    (icons::STAR, "Favorites"),
    (icons::PLAYLIST, "Playlists"),
    (icons::RESTART, "Recently played"),
];

/// What a right-click on a track offers. Reported, never performed — the same
/// line the menubar draws.
const TRACK_ACTIONS: [&str; 5] = [
    "Play next",
    "Add to queue",
    "Start track radio",
    "Go to artist",
    "Go to album",
];

/// `3:42`. Times are the one thing on this page a reader checks against a
/// stopwatch, so they are formatted rather than approximated.
fn clock(seconds: f32) -> String {
    let seconds = seconds.max(0.0).round() as u32;
    format!("{}:{:02}", seconds / 60, seconds % 60)
}

/// Which of the three volume glyphs a level wants. One Solar family, so the
/// speaker cone keeps its shape as the slider moves.
fn volume_icon(level: f32) -> &'static str {
    if level <= 0.0 {
        icons::VOLUME_MUTE
    } else if level < 0.5 {
        icons::VOLUME_LOW
    } else {
        icons::VOLUME_LOUD
    }
}

pub struct MusicPlayer {
    /// The playback clock. `position` is where the track sat at `position_at`,
    /// and while playing the elapsed time is *derived* from the two — see
    /// [`Self::elapsed`].
    playing: bool,
    position: f32,
    position_at: web_time::Instant,
    /// Where the scrubber is being held, in seconds. `Some` detaches the
    /// display from the clock for exactly as long as the pointer is down.
    scrub: Option<f32>,
    track: usize,
    volume: f32,
    shuffle: bool,
    /// Off, all, one. A three-state toggle, so it counts rather than flips.
    repeat: usize,
    liked: HashSet<usize>,
    library: usize,
    /// Right-click on a track: which one, and where the click landed.
    menu: popover::Popup<(usize, gpui::Point<gpui::Pixels>)>,
    scroll: gpui::ScrollHandle,
    bar: bezel_ui::scroll::ScrollbarState,
}

/// The page has exactly one starting state and takes no arguments to reach it,
/// so this is `Default` rather than the `new(cx)` its component siblings need.
impl Default for MusicPlayer {
    fn default() -> Self {
        Self {
            // Starts paused. The clock only advances because render asks for
            // the next frame, so at rest this page costs nothing.
            playing: false,
            position: 0.0,
            position_at: web_time::Instant::now(),
            scrub: None,
            track: 0,
            volume: 0.7,
            shuffle: false,
            repeat: 0,
            liked: [0, 4].into_iter().collect(),
            library: 0,
            menu: popover::Popup::default(),
            scroll: gpui::ScrollHandle::new(),
            bar: bezel_ui::scroll::ScrollbarState::new(),
        }
    }
}

impl MusicPlayer {
    /// Seconds into the current track. Derived from the wall clock rather than
    /// accumulated a frame at a time: a dropped frame then costs nothing, and
    /// there is no per-frame arithmetic drifting away from the truth.
    fn elapsed(&self) -> f32 {
        let position = if self.playing {
            self.position + self.position_at.elapsed().as_secs_f32()
        } else {
            self.position
        };
        position.min(self.track_length())
    }

    /// What the transport *shows*. While the scrubber is held this is the grab,
    /// not the clock — otherwise a playing track drags the thumb back out from
    /// under the pointer on every frame, which reads as a seek bar that fights
    /// you.
    fn scrub(&self) -> f32 {
        self.scrub.unwrap_or_else(|| self.elapsed())
    }

    fn track_length(&self) -> f32 {
        TRACKS[self.track].2 as f32
    }

    /// Move the clock to `position` and keep playing state as it was. Every
    /// seek goes through here, so the two fields cannot disagree about when
    /// "now" is.
    fn seek(&mut self, position: f32) {
        self.position = position.clamp(0.0, self.track_length());
        self.position_at = web_time::Instant::now();
    }

    fn toggle_play(&mut self, cx: &mut Context<Self>) {
        self.seek(self.elapsed());
        self.playing = !self.playing;
        cx.notify();
    }

    /// Step tracks, wrapping — a queue is a ring, unlike the tree's document.
    fn step_track(&mut self, delta: isize, cx: &mut Context<Self>) {
        let count = TRACKS.len() as isize;
        self.track = (self.track as isize + delta).rem_euclid(count) as usize;
        self.seek(0.0);
        cx.notify();
    }

    fn play_track(&mut self, index: usize, cx: &mut Context<Self>) {
        self.track = index;
        self.seek(0.0);
        self.playing = true;
        cx.notify();
    }

    fn close_menu(&mut self, cx: &mut Context<Self>) {
        if self.menu.begin_close() {
            popover::reap_popup(cx, |view: &mut Self| &mut view.menu);
        }
    }
}

impl Render for MusicPlayer {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::of(cx).clone();

        // The playback clock is derived at paint time, so the only thing that
        // makes it *move* is asking for the next frame. Paused, this costs
        // nothing — and neither does another page being open, since a view that
        // is not rendered never gets here.
        if self.playing {
            window.request_animation_frame();
        }

        div()
            .relative()
            .size_full()
            .child(
                div()
                    .flex()
                    .flex_row()
                    .size_full()
                    .min_h_0()
                    .child(self.sidebar(&theme, cx))
                    .child(self.track_pane(&theme, cx)),
            )
            // Overlay, never a gutter: the bar floats over the track list and
            // the list keeps its full height under it.
            .child(
                div()
                    .absolute()
                    .bottom(px(20.0))
                    .left_0()
                    .right_0()
                    .flex()
                    .justify_center()
                    // The cap is what keeps it floating: without it a wide
                    // window stretches the bar edge to edge and it reads as a
                    // dock instead.
                    .child(
                        div()
                            .w_full()
                            .max_w(px(880.0))
                            .child(self.transport(&theme, cx)),
                    ),
            )
            // Mounted from here rather than from the gallery's root, which is
            // the whole point of the entity: `menu_at` anchors to the window,
            // so a nested view can carry its own popup.
            .when(self.menu.get().is_some(), |page| {
                page.child(self.context_menu(&theme, cx))
            })
    }
}

impl MusicPlayer {
    fn sidebar(&mut self, theme: &Theme, cx: &mut Context<Self>) -> AnyElement {
        div()
            .w(px(240.0))
            .flex_none()
            .h_full()
            .pr(px(20.0))
            .flex()
            .flex_col()
            .gap(px(10.0))
            .child(theme.field_label("Library"))
            .child(theme.group_box().children(LIBRARY.iter().enumerate().map(
                |(index, (icon, label))| {
                    theme
                        .card_row(index == 0)
                        .hover(widgets::card_row_hover)
                        .id(SharedString::from(format!("library-{index}")))
                        .cursor_pointer()
                        .when(index == self.library, |row| {
                            row.bg(bezel_theme::card_selected_bg())
                        })
                        .on_click(cx.listener(move |view, _, _, cx| {
                            view.library = index;
                            cx.notify();
                        }))
                        .child(theme.row_tile(icon))
                        .child(theme.row_title(*label))
                        .into_any_element()
                },
            )))
            .into_any_element()
    }

    fn track_pane(&mut self, theme: &Theme, cx: &mut Context<Self>) -> AnyElement {
        let columns = track_columns();
        let playing = self.track;

        div()
            .flex_1()
            .min_w_0()
            .h_full()
            .flex()
            .flex_col()
            .gap(px(16.0))
            .child(self.album_header(theme))
            .child(
                div()
                    .relative()
                    .flex_1()
                    .min_h_0()
                    .child(
                        div()
                            .id("music-tracks")
                            .size_full()
                            .overflow_y_scroll()
                            .track_scroll(&self.scroll)
                            .child(
                                table::table(theme)
                                    .child(table::header(theme).children(columns.iter().map(
                                        |column| {
                                            table::header_cell(theme, column, None)
                                                .into_any_element()
                                        },
                                    )))
                                    .children(TRACKS.iter().enumerate().map(
                                        |(index, (title, artist, seconds))| {
                                            self.track_row(
                                                theme, &columns, index, playing, title, artist,
                                                *seconds, cx,
                                            )
                                        },
                                    ))
                                    // Clear of the floating bar, so the last
                                    // track is reachable rather than parked
                                    // under the bar.
                                    .pb(px(BAR_HEIGHT + 40.0)),
                            ),
                    )
                    .child(scroll::scrollbar("music-bar", &self.scroll, &self.bar)),
            )
            .into_any_element()
    }

    fn album_header(&self, theme: &Theme) -> AnyElement {
        let total: u32 = TRACKS.iter().map(|(_, _, seconds)| seconds).sum();

        div()
            .flex()
            .flex_row()
            .items_end()
            .gap(px(18.0))
            .child(artwork(theme, 128.0, 14.0))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(6.0))
                    .child(
                        div()
                            .text_size(px(11.0))
                            .text_color(theme.text_faint)
                            .child(popover::tracked_upper("Album")),
                    )
                    .child(
                        div()
                            .text_size(px(28.0))
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .child(ALBUM),
                    )
                    .child(theme.meta_line(vec![
                        SharedString::from(ARTIST).into_any_element(),
                        SharedString::from(format!("{} tracks", TRACKS.len())).into_any_element(),
                        SharedString::from(format!("{} min", total / 60)).into_any_element(),
                    ])),
            )
            .into_any_element()
    }

    #[allow(clippy::too_many_arguments)]
    fn track_row(
        &self,
        theme: &Theme,
        columns: &[Column],
        index: usize,
        playing: usize,
        title: &'static str,
        artist: &'static str,
        seconds: u32,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let current = index == playing;
        let liked = self.liked.contains(&index);

        table::row(
            theme,
            columns,
            index == 0,
            current,
            vec![
                div()
                    .text_color(if current {
                        theme.accent
                    } else {
                        theme.text_faint
                    })
                    .font_family(theme.font_mono.clone())
                    .child(SharedString::from(format!("{}", index + 1)))
                    .into_any_element(),
                div()
                    .flex()
                    .flex_col()
                    .min_w_0()
                    .child(
                        div()
                            .text_color(if current { theme.accent } else { theme.text })
                            .child(title),
                    )
                    .child(
                        div()
                            .text_size(px(11.0))
                            .text_color(theme.text_faint)
                            .child(artist),
                    )
                    .into_any_element(),
                div()
                    .id(SharedString::from(format!("like-{index}")))
                    .cursor_pointer()
                    // Inside a row that plays on click, so the press has to
                    // stop here or liking a track also starts it.
                    .on_click(cx.listener(move |view, _, _, cx| {
                        if !view.liked.remove(&index) {
                            view.liked.insert(index);
                        }
                        cx.stop_propagation();
                        cx.notify();
                    }))
                    .child(
                        icons::icon(if liked {
                            icons::HEART_BOLD
                        } else {
                            icons::HEART
                        })
                        .size(px(14.0))
                        .text_color(if liked {
                            theme.accent
                        } else {
                            theme.text_faint
                        }),
                    )
                    .into_any_element(),
                div()
                    .font_family(theme.font_mono.clone())
                    .text_color(theme.text_muted)
                    .child(SharedString::from(clock(seconds as f32)))
                    .into_any_element(),
            ],
        )
        .id(SharedString::from(format!("track-{index}")))
        .cursor_pointer()
        .on_click(cx.listener(move |view, _, _, cx| view.play_track(index, cx)))
        // The root opens the gallery's own context menu on right-click, and
        // this row is inside it: without stopping the press here both menus
        // mount and the generic one wins the pointer.
        .on_mouse_down(
            MouseButton::Right,
            cx.listener(move |view, event: &MouseDownEvent, _, cx| {
                view.menu.open((index, event.position));
                cx.stop_propagation();
                cx.notify();
            }),
        )
        .into_any_element()
    }

    /// The bar. Five controls on the left and three on the right — deliberately
    /// asymmetric, because that asymmetry is what the centring rule is for.
    fn transport(&mut self, theme: &Theme, cx: &mut Context<Self>) -> AnyElement {
        let (title, artist, seconds) = TRACKS[self.track];
        let position = self.scrub();
        let length = seconds as f32;

        // `lit` is the whole state vocabulary of a transport: shuffle and repeat
        // are toggles that have to read as on from across the room, and
        // everything else is quiet.
        let icon_button = |id: &'static str, path: &'static str, size: f32, lit: bool| {
            let hover = theme.glass_hover();
            let tint = if lit { theme.accent } else { theme.text_muted };
            control_bar::bar_button(path, size, tint)
                .id(id)
                .hover(move |s| s.bg(hover))
        };

        let leading = vec![
            icon_button("bar-shuffle", icons::SHUFFLE, 32.0, self.shuffle)
                .on_click(cx.listener(|view, _, _, cx| {
                    view.shuffle = !view.shuffle;
                    cx.notify();
                }))
                .into_any_element(),
            icon_button("bar-prev", icons::SKIP_PREVIOUS, 32.0, false)
                .on_click(cx.listener(|view, _, _, cx| view.step_track(-1, cx)))
                .into_any_element(),
            // The primary action: bigger, and the one glyph painted at full
            // text weight rather than muted.
            control_bar::bar_button(
                if self.playing {
                    icons::PAUSE_BOLD
                } else {
                    icons::PLAY_BOLD
                },
                40.0,
                theme.text,
            )
            .id("bar-play")
            .hover({
                let hover = theme.glass_hover();
                move |s| s.bg(hover)
            })
            .on_click(cx.listener(|view, _, _, cx| view.toggle_play(cx)))
            .into_any_element(),
            icon_button("bar-next", icons::SKIP_NEXT, 32.0, false)
                .on_click(cx.listener(|view, _, _, cx| view.step_track(1, cx)))
                .into_any_element(),
            icon_button(
                "bar-repeat",
                if self.repeat == 2 {
                    icons::REPEAT_ONE
                } else {
                    icons::REPEAT
                },
                32.0,
                self.repeat > 0,
            )
            .on_click(cx.listener(|view, _, _, cx| {
                view.repeat = (view.repeat + 1) % 3;
                cx.notify();
            }))
            .into_any_element(),
        ];

        let centre = div()
            .w(px(300.0))
            .flex()
            .flex_row()
            .items_center()
            .gap(px(10.0))
            .child(artwork(theme, 36.0, 6.0))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .gap(px(3.0))
                    .child(
                        div()
                            .text_size(px(12.0))
                            .font_weight(gpui::FontWeight::MEDIUM)
                            .child(title),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(px(6.0))
                            .text_size(px(10.0))
                            .font_family(theme.font_mono.clone())
                            .text_color(theme.text_faint)
                            .child(SharedString::from(clock(position)))
                            .child(
                                div()
                                    .flex_1()
                                    .id("bar-scrub")
                                    .child(theme.slider(position / length))
                                    .on_drag(SliderDrag, |_, _, _, cx| cx.new(|_| gpui::Empty))
                                    .on_drag_move(cx.listener(
                                        move |view, event: &DragMoveEvent<SliderDrag>, _, cx| {
                                            let fraction = widgets::axis_fraction(
                                                event.event.position,
                                                event.bounds,
                                                Axis::Horizontal,
                                                0.0,
                                            );
                                            view.scrub = Some(fraction * length);
                                            cx.notify();
                                        },
                                    ))
                                    // The commit. gpui delivers no drag-end, so
                                    // the release lands here — and the grab has
                                    // to clear whether or not it moved, or the
                                    // display stays detached from the clock.
                                    .on_mouse_up(
                                        MouseButton::Left,
                                        cx.listener(move |view, _, _, cx| {
                                            if let Some(position) = view.scrub.take() {
                                                view.seek(position);
                                            }
                                            cx.notify();
                                        }),
                                    ),
                            )
                            .child(SharedString::from(clock(length))),
                    )
                    .child(
                        div()
                            .text_size(px(10.0))
                            .text_color(theme.text_faint)
                            .child(artist),
                    ),
            )
            .into_any_element();

        let trailing = vec![
            icon_button("bar-lyrics", icons::MICROPHONE, 32.0, false).into_any_element(),
            icon_button("bar-queue", icons::PLAYLIST, 32.0, false).into_any_element(),
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(6.0))
                .child(
                    icon_button("bar-volume", volume_icon(self.volume), 32.0, false).on_click(
                        cx.listener(|view, _, _, cx| {
                            view.volume = if view.volume > 0.0 { 0.0 } else { 0.7 };
                            cx.notify();
                        }),
                    ),
                )
                .child(
                    div()
                        .id("bar-volume-slider")
                        .w(px(72.0))
                        .child(theme.slider(self.volume))
                        .on_drag(SliderDrag, |_, _, _, cx| cx.new(|_| gpui::Empty))
                        .on_drag_move(cx.listener(
                            |view, event: &DragMoveEvent<SliderDrag>, _, cx| {
                                view.volume = widgets::axis_fraction(
                                    event.event.position,
                                    event.bounds,
                                    Axis::Horizontal,
                                    0.0,
                                );
                                cx.notify();
                            },
                        )),
                )
                .into_any_element(),
        ];

        control_bar::control_bar(theme, Shape::Pill, leading, Some(centre), trailing)
    }

    /// The track context menu, mounted from the root beside the gallery's own.
    fn context_menu(&self, theme: &Theme, cx: &mut Context<Self>) -> AnyElement {
        let Some((index, position)) = self.menu.get().copied() else {
            return div().into_any_element();
        };

        popover::menu_at(
            "music-track-menu",
            position,
            popover::popover_card(theme)
                .w(px(190.0))
                // The heading is what the state in the popup is *for*: a menu
                // that could not name its track would not need to carry one.
                .child(popover::menu_heading(theme, TRACKS[index].0))
                .children(TRACK_ACTIONS.iter().enumerate().map(|(item, label)| {
                    popover::menu_row(
                        theme,
                        false,
                        SharedString::from(format!("track-menu-{item}")),
                    )
                    .id(SharedString::from(format!("track-menu-item-{item}")))
                    .on_click(cx.listener(|view, _, _, cx| view.close_menu(cx)))
                    .child(*label)
                    .into_any_element()
                }))
                .into_any_element(),
            self.menu.closing_since(),
        )
    }
}

/// The track list's columns, declared once and shared by the header and every
/// row — the same rule the table page is built on.
fn track_columns() -> Vec<Column> {
    vec![
        Column::new("#", Width::Fixed(px(36.0))),
        Column::new("Title", Width::Flex(1.0)),
        Column::new("", Width::Fixed(px(32.0))),
        Column::new("Time", Width::Fixed(px(56.0))).align_end(),
    ]
}

/// The artwork slot. A gradient, not an image: gpui already has `img()`, and
/// wrapping it would add nothing but a place for an app's own decode and cache
/// to go wrong. Swap this one div for `img(source).size_full()`.
fn artwork(theme: &Theme, size: f32, radius: f32) -> gpui::Div {
    div()
        .flex_none()
        .size(px(size))
        .rounded(px(radius))
        .border_1()
        .border_color(theme.border)
        .bg(linear_gradient(
            150.0,
            linear_color_stop(theme.accent, 0.0),
            linear_color_stop(theme.warning, 1.0),
        ))
}
