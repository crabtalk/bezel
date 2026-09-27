//! The gallery — bezel's documentation, and its dev surface: a rail of every
//! component on the left, the selected one in the pane.
//!
//! [`TABS`] is the catalog: a top nav for the *kind* of thing, a rail for the
//! items in it. A component lands here the day it lands in `crates/ui`,
//! composed exactly once, so the browser is never out of date with the library
//! it documents.

use gpui::{
    AnyElement, App, Axis, Context, DragMoveEvent, Empty, Entity, KeyBinding, SharedString, Window,
    actions, div, point, prelude::*, px, relative,
};
use motion::{AppExt as _, Fade, Painter};
use rail::{Rail, Selected};
use std::{cell::Cell, collections::HashSet, rc::Rc};
use theme::{
    ControlSize, Glass, Material, Sizing, SurfaceSpec, SurfaceStyle, TextStyle, Theme, Typeset,
    appearance::{self, AppearanceMode},
};
use ui::{
    combobox::{self, Combobox},
    control_bar::{self, Shape as ControlBarShape},
    date::{self, Calendar, Date},
    floating::{self, Floating},
    focus,
    hover_card::HoverCard,
    icons::{self, Icon},
    input::{self, Shape, TextField},
    keys, list, loaders,
    menu::Item,
    menubar::{self, Menu, Menubar, MenubarEvent},
    pagination,
    palette::{self, CommandPalette, PaletteEvent},
    pending::PendingKeys,
    popover,
    scroll::{self, Axes, ScrollbarState, TransientState},
    stats::{self, Stats},
    surface::Surfaced as _,
    table::{self, Column, Sort, Width},
    tabs, titlebar,
    tooltip::Tooltip,
    tree::{self, Direction, Move},
    widgets,
    widgets::{
        ButtonStyle, Buttons, Content, Controls, Icons as _, Layout, Scaffolding, SliderDrag,
        SplitDrag, SplitStyle, Status,
    },
};

actions!(
    gallery,
    [
        OpenPalette,
        ToggleFullScreen,
        CloseOverlay,
        ToggleFpsOverlay,
        ResetFrameOverlayStats,
        Quit
    ]
);

/// Whether this build owns the window's menus. macOS has the system bar, and
/// a browser tab has no window to quit.
const APP_MENUBAR: bool = !cfg!(any(target_os = "macos", target_family = "wasm"));

/// Every keymap this view needs, in one call.
///
/// Two entry points open it — a native window and a browser tab — and a list
/// each of them keeps by hand is a list they drift out of: the editor's
/// bindings were installed natively and missing on the web, so typing worked in
/// the browser and Backspace did not.
pub fn init(cx: &mut App) {
    markdown::set_highlighter(cx, highlight::spans, highlight::languages());
    markdown::set_link_preview(cx, preview::of);
    markdown::set_block_renderer(cx, blocks::render);
    // The dialect this gallery reads and writes: two marks CommonMark has no
    // spelling for, registered rather than waited on. See the Ribbon page.
    markdown::set_marks(
        cx,
        markdown::Marks::new()
            .with("highlight", "==")
            .with("underline", "++"),
    );
    markdown::set_mark_paint(cx, patterns::ribbon::paint);
    editor::set_image_store(cx, store::of());
    input::init(cx);
    editor::init(cx);
    canvas::init(cx);
    palette::init(cx);
    combobox::init(cx);
    date::init(cx);
    focus::init(cx);
    menubar::init(cx);
    tree::init(cx);
    // A pattern is an app: the composer page binds its own keys.
    patterns::agent::init(cx);
    patterns::browser::init(cx);
    cx.bind_keys([
        #[cfg(target_os = "macos")]
        KeyBinding::new("cmd-k", OpenPalette, None),
        #[cfg(not(target_os = "macos"))]
        KeyBinding::new("ctrl-k", OpenPalette, None),
        // Scoped to this page's context, so it never shadows the escape a
        // menu, a combobox or the document editor binds inside its own.
        KeyBinding::new("escape", CloseOverlay, Some("Gallery")),
        // Sequences, for the pending-keys hint in the corner.
        KeyBinding::new("ctrl-g p", OpenPalette, Some("Gallery")),
        KeyBinding::new("ctrl-g f", ToggleFullScreen, Some("Gallery")),
    ]);
}

pub mod highlight;

pub mod brand;
pub mod patterns;
pub mod preview;
mod rail;
pub mod store;

mod catalog;
mod chrome;
mod fixtures;
mod sections;

pub use catalog::*;
pub use fixtures::*;
use sections::*;
use sections::{controls, data, foundations, material, navigation, overlays};

/// The rail's padding — the grid its rows, its footer and the traffic lights
/// all sit on.
pub(crate) const RAIL_PAD: f32 = 20.0;

/// The rail's width, and the padding inside the content card. Read together
/// they are the window's two columns: the nav strip aligns to the card's grid
/// so its tabs sit over the section header and its switch over the header's
/// trailing edge.
pub(crate) const RAIL_WIDTH: f32 = 200.0;
const CARD_PAD: f32 = 24.0;

/// The pane's own padding around the column, and the width that column is.
const PANE_PAD: f32 = 32.0;
const COLUMN_WIDTH: f32 = 420.0;

/// Below this the two-column form does not fit — the rail plus the padded
/// column every component page is designed for. Derived rather than chosen, so
/// moving either width moves the breakpoint with it.
const COMPACT_BELOW: f32 = RAIL_WIDTH + PANE_PAD * 2.0 + COLUMN_WIDTH;

/// The floor under a pattern's canvas once it is being panned rather than
/// fitted: the narrowest pane the two-column layout ever hands one.
const CANVAS_MIN: f32 = COMPACT_BELOW - RAIL_WIDTH;

/// Padding inside one nav tab, subtracted back out of the strip so the tabs'
/// text starts on the card's grid rather than their hit boxes.
const NAV_ITEM_PAD: f32 = 4.0;

/// macOS traffic light diameter — measured 2026-08-27 on macOS 26. AppKit owns
/// the buttons and gpui reads their frame off the live views, so no constant
/// here can derive it.
const TRAFFIC_LIGHT_SIZE: f32 = 14.0;

/// The traffic lights' top-left corner, passed to
/// `TitlebarOptions::traffic_light_position`: the rail's grid across, the nav
/// strip's centre down. macOS sizes the button container to `height + 2y`, so
/// the `y` that centres them is the one making that container the strip.
pub const TRAFFIC_LIGHT_X: f32 = RAIL_PAD;
pub const TRAFFIC_LIGHT_Y: f32 = (Theme::HEADER_HEIGHT - TRAFFIC_LIGHT_SIZE) / 2.0;

/// Centre-to-centre spacing of the three buttons — measured 2026-08-30 on macOS
/// 26, off a live window's `AXButton` frames. Same reason as the diameter above:
/// AppKit owns the cluster and gpui reports no frame for it.
const TRAFFIC_LIGHT_PITCH: f32 = 23.0;

/// What the nav owes the leading edge once the rail is a drawer. The wide
/// layout clears the cluster by accident — the rail is wider than it — so this
/// is the only place the cluster's own extent has to be named. A browser tab
/// has no titlebar to clear, and on a phone that inset is a fifth of the width.
const COMPACT_NAV_PAD: f32 = if cfg!(target_os = "macos") {
    TRAFFIC_LIGHT_X + TRAFFIC_LIGHT_PITCH * 2.0 + TRAFFIC_LIGHT_SIZE + RAIL_PAD
} else {
    RAIL_PAD
};

pub struct Gallery {
    /// Mounted only while open — a palette that lingers keeps a stale query.
    palette: Option<Entity<CommandPalette>>,
    last_command: Option<SharedString>,
    /// Built on the first render, which is the first time there is a window.
    pending: Option<Entity<PendingKeys>>,
    /// Right-click menu, anchored at the click position.
    context_menu: popover::Popup<gpui::Point<gpui::Pixels>>,
    /// The menubar demo, which holds which menu is down.
    menubar: Entity<Menubar>,
    /// The app's own menus, mounted in the nav where the platform has no menu
    /// bar of its own. A second entity rather than a second mount of the one
    /// above: two bars showing one open-menu state would open together.
    app_menus: Entity<Menubar>,
    /// Whether a press on the nav is still a candidate for a window move.
    drag: titlebar::DragState,
    /// What it last reported. The bar keeps no selection — a menu item is an
    /// action, not a value — so the host is where the answer lands.
    last_menu_item: Option<SharedString>,
    /// Which edge the open sheet is pinned to — the page offers both, and
    /// the mount below reads it back rather than keeping a second flag.
    sheet: popover::Popup<popover::Side>,
    /// The rail, when the window is too narrow to carry it beside the pane.
    drawer: popover::Popup<()>,
    /// The window's resting focus. Without it the key context has no node in
    /// the focus path, and `cmd-k` reaches nothing.
    focus_handle: gpui::FocusHandle,
    /// The composer's knobs, and which of its three files is showing. What they
    /// are *set to* is the brand global — the page keeps no palette.
    brand_knobs: [gpui::FocusHandle; brand::KNOB_COUNT],
    brand_file: usize,
    /// Which button was last pressed, and by what — the only way to see that a
    /// keyboard press and a click reach the same place.
    last_pressed: Option<SharedString>,
    /// The rail is its own view so it can be cached: forty rows that change
    /// only on a tab or a selection, and were being rebuilt every frame.
    rail: Entity<Rail>,
    pane_scroll: gpui::ScrollHandle,
    pane_bar: TransientState,
    /// Which top-nav tab is open.
    tab: usize,
    /// Where you were in each tab — switching away and back should land you
    /// where you left, not at the top.
    selected: Vec<&'static str>,
    dialog: popover::Popup<()>,
    /// The frame meter, mounted once and floated over whichever page is open.
    /// One instance: two of them would each count the other's frames, and
    /// neither would ever read zero.
    stats: Entity<Stats>,
    stats_shown: bool,
    /// Where the meter has been dragged to, if it has.
    stats_at: Floating,
    /// Renders one section alone, without the nav, rail or header around it.
    /// The website embeds a page per component this way, so a doc page shows
    /// the component it documents rather than the whole browser.
    embedded: bool,
    /// Which [`Example`] the embed is showing, of the section it opened on.
    /// `None` is the whole page. A doc page holds one embed and moves it from
    /// snippet to snippet, so this changes without the window reloading.
    example: Option<SharedString>,
    controls: controls::State,
    data: data::State,
    navigation: navigation::State,
    overlays: overlays::State,
    material: material::State,
    foundations: foundations::State,
    patterns: patterns::State,
}

impl Gallery {
    pub fn new(cx: &mut Context<Self>) -> Self {
        // Opened on the same page the fields below start at.
        let rail = cx.new(|cx| Rail::new(2, TABS[2].home, cx));
        cx.subscribe(&rail, |view, _, selected: &Selected, cx| {
            view.open(view.tab, selected.0, cx);
            // A drawer that stays up over the page it just opened is a drawer
            // you have to dismiss to see what you picked.
            view.close_drawer(cx);
        })
        .detach();
        // The bar reports a place in the menus it was given; the host is what
        // turns that back into a name, and what decides it means anything.
        let menubar = cx.new(|cx| Menubar::new(demo_menus(), cx));
        cx.subscribe(&menubar, |view, bar, event, cx| {
            let MenubarEvent::Selected { menu, path } = event;
            if let Some(Item::Action { label, .. }) = bar.read(cx).menus()[*menu].at(path) {
                view.last_menu_item = Some(label.clone());
            }
            cx.notify();
        })
        .detach();

        // The two rows `cx.set_menus` gives macOS, for the platforms it is not
        // called on. Printed off the keymap, so the chord is the bound one.
        let app_menus = cx.new(|cx| {
            Menubar::new(
                vec![
                    Menu::new("bezel", vec![Item::action("Quit")]),
                    Menu::new("Window", vec![Item::action("Toggle Full Screen")]),
                ],
                cx,
            )
        });
        cx.subscribe(&app_menus, |_, bar, event, cx| {
            let MenubarEvent::Selected { menu, path } = event;
            let Some(Item::Action { label, .. }) = bar.read(cx).menus()[*menu].at(path) else {
                return;
            };
            // Through the keymap rather than run here, so the menu and the
            // chord reach the same handler.
            match label.as_ref() {
                "Quit" => cx.dispatch_action(&Quit),
                "Toggle Full Screen" => cx.dispatch_action(&ToggleFullScreen),
                _ => {}
            }
        })
        .detach();

        Self {
            menubar,
            app_menus,
            drag: titlebar::DragState::default(),
            last_menu_item: None,
            palette: None,
            last_command: None,
            pending: None,
            context_menu: popover::Popup::default(),
            sheet: popover::Popup::default(),
            drawer: popover::Popup::default(),
            focus_handle: cx.focus_handle(),
            brand_knobs: std::array::from_fn(|_| cx.focus_handle()),
            brand_file: 0,
            rail,
            pane_scroll: gpui::ScrollHandle::new(),
            pane_bar: TransientState::new(Painter::of(cx)),
            last_pressed: None,
            tab: 2,
            selected: TABS.iter().map(|tab| tab.home).collect(),
            dialog: popover::Popup::default(),
            stats: cx.new(Stats::new),
            stats_shown: false,
            stats_at: Floating::new(Painter::of(cx)),
            embedded: false,
            example: None,
            controls: controls::State::new(cx),
            data: data::State::new(cx),
            navigation: navigation::State::new(cx),
            overlays: overlays::State::new(cx),
            material: material::State::new(cx),
            foundations: foundations::State::new(cx),
            patterns: patterns::State::new(cx),
        }
    }

    /// The one place a page is opened. Everything that changes what the browser
    /// shows goes through here, because the rail is cached: it repaints when it
    /// is told to, and a notify raised mid-render never reaches it — the frame
    /// it would have to arrive in has already decided the rail is clean.
    fn open(&mut self, tab: usize, key: &'static str, cx: &mut Context<Self>) {
        self.tab = tab;
        self.selected[tab] = key;
        self.rail.update(cx, |rail, cx| rail.show(tab, key, cx));
        cx.notify();
    }

    /// Show or hide the meter, and tell the app what that means for animation.
    ///
    /// A meter that stops the moment you look at something else cannot report
    /// what the app costs in the background, which is the case worth catching.
    /// So while it is up the app keeps animating unfocused — the reading and
    /// the thing being read are the same cost, and that is the honest trade.
    fn show_stats(&mut self, shown: bool, cx: &mut Context<Self>) {
        self.stats_shown = shown;
        cx.set_pause_when_inactive(!shown);
        cx.notify();
    }

    /// The browser, opened on one section — `cargo run -p gallery -- editor`.
    /// Falls back to the default page when the key is not in the catalog, so a
    /// stale link lands somewhere useful instead of on an empty pane.
    pub fn showing(key: &str, cx: &mut Context<Self>) -> Self {
        let mut gallery = Self::new(cx);
        if let Some(tab) = tab_of(key) {
            gallery.open(tab, section_at(key).expect("tab_of matched").key, cx);
        }
        gallery
    }

    /// That section *alone*, for a website page that documents it — no rail, no
    /// tabs, just the pane. `example` narrows it further to one demo on the
    /// page, which is what a doc page's preview shows.
    pub fn embedded(key: &str, example: Option<&str>, cx: &mut Context<Self>) -> Self {
        let mut gallery = Self::showing(key, cx);
        gallery.embedded = tab_of(key).is_some();
        gallery.example = example.map(SharedString::from);
        gallery
    }

    /// Point the embed at another example of the section it is already on —
    /// what a doc page sends as the reader scrolls from snippet to snippet.
    /// `None` widens it back to the whole page.
    pub fn show_example(&mut self, example: Option<&str>, cx: &mut Context<Self>) {
        let example = example.map(SharedString::from);
        if self.example == example {
            return;
        }
        self.example = example;
        cx.notify();
    }

    /// The gallery's own focus handle — what the window focuses on launch.
    pub fn focus_handle(&self) -> gpui::FocusHandle {
        self.focus_handle.clone()
    }

    fn open_palette(&mut self, _: &OpenPalette, window: &mut Window, cx: &mut Context<Self>) {
        let palette = cx.new(|cx| {
            CommandPalette::new(
                COMMANDS.iter().map(|c| SharedString::from(*c)).collect(),
                cx,
            )
        });
        // The host decides what a selection means; the palette only reports.
        cx.subscribe(&palette, |view, _, event, cx| {
            match event {
                PaletteEvent::Selected(index) => {
                    view.last_command = Some(SharedString::from(COMMANDS[*index]));
                }
                PaletteEvent::Dismissed => {}
            }
            view.palette = None;
            cx.notify();
        })
        .detach();
        palette.update(cx, |palette, cx| palette.focus(window, cx));
        self.palette = Some(palette);
        cx.notify();
    }

    /// `ctrl-alt-shift-p` — zed's own chord for the same overlay, cycling
    /// hidden → last frame's draw time → percentiles and total frame count.
    ///
    /// The number to watch is FRAMES while nothing on screen is moving: a
    /// window at rest should hold it still. gpui paints the panel as raw quads
    /// straight into the scene, skipping layout, text and invalidation, so
    /// reading the count does not add to it.
    ///
    /// Built on `--features profiler`; the histograms behind it are not free,
    /// so without that flag the whole affordance compiles out.
    fn toggle_fps_overlay(
        &mut self,
        _: &ToggleFpsOverlay,
        _window: &mut Window,
        _: &mut Context<Self>,
    ) {
        #[cfg(feature = "profiler")]
        _window.cycle_debug_frame_overlay_mode();
    }

    /// `ctrl-alt-shift-o`. Clears the percentile window so the next reading
    /// describes what you are about to do rather than the scrolling you did to
    /// reach it. The total frame count survives on purpose.
    fn reset_frame_overlay_stats(
        &mut self,
        _: &ResetFrameOverlayStats,
        _window: &mut Window,
        _: &mut Context<Self>,
    ) {
        #[cfg(feature = "profiler")]
        _window.reset_debug_frame_overlay_stats();
    }

    /// `ctrl-cmd-f`. macOS draws that shortcut on the Window menu of a nib-built
    /// app; a gpui app has no nib, so AppKit supplies nothing and the keystroke
    /// reaches whatever the app binds — which, until this existed, was nothing.
    /// Zed carries the same binding in its own keymap for the same reason.
    fn toggle_full_screen(
        &mut self,
        _: &ToggleFullScreen,
        window: &mut Window,
        _: &mut Context<Self>,
    ) {
        window.toggle_fullscreen();
    }

    /// What a button does, reached by click and by `enter`/`space` alike.
    fn press(&mut self, label: &'static str, cx: &mut Context<Self>) {
        self.last_pressed = Some(SharedString::from(label));
        cx.notify();
    }

    fn nudge(&mut self, delta: f32, cx: &mut Context<Self>) {
        self.controls.level = (self.controls.level + delta).clamp(0.0, 1.0);
        cx.notify();
    }

    fn close_context_menu(&mut self, cx: &mut Context<Self>) {
        if self.context_menu.begin_close() {
            popover::reap_popup(self, cx, |view: &mut Self| &mut view.context_menu);
        }
        cx.notify();
    }

    /// Straight off, because `popover::modal` has no exit to play — unlike the
    /// sheet and the context menu, it takes no `closing` and paints the same
    /// whether or not one has begun.
    fn close_dialog(&mut self, cx: &mut Context<Self>) {
        self.dialog.close();
        cx.notify();
    }

    /// Escape, on whatever is floating over the page.
    ///
    /// `popover::modal` and `popover::sheet` are paint, not entities — they
    /// hold no state and so can bind no key. The state is the caller's
    /// `Popup`, which makes backing out of one the caller's too, and every
    /// overlay this page can open answers here.
    fn close_overlay(&mut self, _: &CloseOverlay, _: &mut Window, cx: &mut Context<Self>) {
        self.close_dialog(cx);
        self.close_sheet(cx);
        self.close_drawer(cx);
        self.close_context_menu(cx);
    }

    fn close_sheet(&mut self, cx: &mut Context<Self>) {
        if self.sheet.begin_close() {
            popover::reap_popup(self, cx, |view: &mut Self| &mut view.sheet);
        }
        cx.notify();
    }

    fn open_drawer(&mut self, cx: &mut Context<Self>) {
        self.drawer.open(());
        cx.notify();
    }

    fn close_drawer(&mut self, cx: &mut Context<Self>) {
        if self.drawer.begin_close() {
            popover::reap_popup(self, cx, |view: &mut Self| &mut view.drawer);
        }
        cx.notify();
    }

    fn choose_theme(&mut self, index: usize, cx: &mut Context<Self>) {
        self.controls.theme_choice = index;
        popover::close_popup(self, cx, |view: &mut Self| &mut view.controls.theme_menu);
        cx.notify();
    }
}
