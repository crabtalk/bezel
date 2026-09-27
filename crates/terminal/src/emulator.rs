//! The terminal emulator core: `alacritty_terminal`'s `Term` + vte's ANSI
//! `Processor` wrapped as a pure state machine.
//!
//! Bytes in ([`Emulator::feed`]), grid snapshots out ([`Emulator::line`],
//! [`Emulator::cursor`]). No timers, no gpui, and no I/O but the graphics
//! files [`Emulator::set_local_media`] lets a program name: the host owns the
//! PTY and scheduling, the view owns paint. That split makes the whole escape-sequence
//! surface unit-testable with scripted byte strings.
//!
//! Selection lives here too ([`Emulator::start_selection`] and friends) rather
//! than in the host, because `Term` is what knows how to keep anchors on their
//! text as output scrolls the grid underneath them.
//!
//! API notes for the pinned `alacritty_terminal 0.26` / `vte 0.15`:
//! - `Processor::advance` consumes a byte slice; `Term` implements the
//!   `vte::ansi::Handler` trait directly, so no event-loop machinery is needed.
//! - `Term::new` takes any `grid::Dimensions` impl — [`GridSize`] here.
//! - Query responses (DSR/DA/…) surface as `Event::PtyWrite` on the listener;
//!   [`Emulator::feed`] returns them so the host can write them back.
//! - Kitty graphics never reach the parser at all — `vte` discards APC runs
//!   with no hook to catch them — so [`crate::scanner::Scanner`] takes them off
//!   the stream first and [`Emulator::feed`] hands the rest on unchanged. A
//!   sixel image's data goes the same way: `vte` hands a DCS to handlers
//!   `Term` leaves empty.
//! - `vte` buffers a synchronized update (mode 2026) and replays it at ESU.
//!   Graphics leave the stream before that buffer, so a replay lands an image
//!   under text written after it. [`NoSync`] turns the buffering off and the
//!   frame is held here instead: see [`Emulator::render_hold`].

use std::{
    cell::RefCell,
    path::{Path, PathBuf},
    rc::Rc,
    time::Duration,
};

use crate::kitty;
use crate::scanner::{Iterm, Scanner, Segment};
use alacritty_terminal::{
    event::{Event, EventListener, WindowSize},
    grid::{Dimensions, Scroll},
    index::{Column, Line, Point},
    selection::{Selection, SelectionRange},
    term::{Config, Term, TermMode, cell::Flags},
    vte::ansi::{Color as AnsiColor, CursorShape, NamedColor, Processor, Rgb as AnsiRgb, Timeout},
};

/// Grid coordinates and selection granularity, re-exported so the host and view
/// speak the emulator's vocabulary without depending on `alacritty_terminal`
/// directly — the same seam [`CellColor`] draws for colors.
pub use alacritty_terminal::index::{Point as GridPoint, Side};
pub use alacritty_terminal::selection::SelectionType;

mod graphics;
mod selection;

use graphics::*;

/// Scrollback history kept client-side (lines). The host's replay window is
/// bounded separately; this only caps what stays scrollable in the UI.
pub const SCROLLBACK_LINES: usize = 10_000;

/// Viewport dimensions in cells.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GridSize {
    pub cols: u16,
    pub rows: u16,
}

impl GridSize {
    pub fn new(cols: u16, rows: u16) -> Self {
        Self {
            cols: cols.max(2),
            rows: rows.max(1),
        }
    }
}

impl Dimensions for GridSize {
    fn total_lines(&self) -> usize {
        self.rows as usize
    }
    fn screen_lines(&self) -> usize {
        self.rows as usize
    }
    fn columns(&self) -> usize {
        self.cols as usize
    }
}

/// A cell's paint color, decoupled from the palette: the view resolves these
/// against the theme (default fg/bg, 256-color index, or direct RGB).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CellColor {
    /// Default foreground.
    Foreground,
    /// Default background.
    Background,
    /// Indexed color: 0-15 ANSI, 16-231 color cube, 232-255 grayscale ramp.
    Indexed(u8),
    /// Direct 24-bit color.
    Rgb(u8, u8, u8),
}

fn map_color(color: AnsiColor) -> CellColor {
    match color {
        AnsiColor::Spec(AnsiRgb { r, g, b }) => CellColor::Rgb(r, g, b),
        AnsiColor::Indexed(ix) => CellColor::Indexed(ix),
        AnsiColor::Named(named) => {
            let ix = named as usize;
            if ix < 16 {
                return CellColor::Indexed(ix as u8);
            }
            match named {
                NamedColor::Background => CellColor::Background,
                // Dim named colors fold onto their base index; the DIM flag
                // still travels on the cell for paint-time dimming.
                NamedColor::DimBlack
                | NamedColor::DimRed
                | NamedColor::DimGreen
                | NamedColor::DimYellow
                | NamedColor::DimBlue
                | NamedColor::DimMagenta
                | NamedColor::DimCyan
                | NamedColor::DimWhite => {
                    CellColor::Indexed((ix - NamedColor::DimBlack as usize) as u8)
                }
                _ => CellColor::Foreground,
            }
        }
    }
}

/// One rendered cell: char + colors + the flags paint cares about.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CellSnapshot {
    pub ch: char,
    pub fg: CellColor,
    pub bg: CellColor,
    pub bold: bool,
    pub dim: bool,
    pub italic: bool,
    pub underline: bool,
    pub inverse: bool,
    pub hidden: bool,
    /// A double-width char (occupies this cell plus the next spacer cell).
    pub wide: bool,
    /// The spacer half of a wide char — never shaped, only background-painted.
    pub wide_spacer: bool,
    /// Inside the active selection: the view paints a wash over this cell.
    pub selected: bool,
}

impl CellSnapshot {
    /// Effective paint colors after INVERSE/HIDDEN resolution.
    pub fn display_colors(&self) -> (CellColor, CellColor) {
        let (fg, bg) = if self.inverse {
            (self.bg, self.fg)
        } else {
            (self.fg, self.bg)
        };
        if self.hidden { (bg, bg) } else { (fg, bg) }
    }
}

/// Where an image sits on the grid: its top-left cell in viewport
/// coordinates, and how many cells it covers.
#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub struct Placement {
    pub row: usize,
    pub col: usize,
    pub cols: u16,
    pub rows: u16,
    /// The [`kitty::Store`] id of the image to paint here.
    pub image: u32,
    /// Where the picture is drawn, in cells from the placement's top-left
    /// cell. It can run past the placement's own cells, which paint clips to.
    pub frame: Frame,
    /// The part of the image drawn into [`Self::frame`].
    pub source: Source,
    /// Negative is under the text, and below `i32::MIN / 2` under
    /// non-default cell backgrounds too. Zero and up is over the text.
    pub z: i32,
}

/// A rectangle in cells, fractional where an offset or a letterbox puts an
/// edge inside one.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Frame {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

/// A rectangle in an image's pixels, inside the image.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Source {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

/// The private-use codepoint a placement is anchored with, plus its id.
///
/// The anchor is a zerowidth char written onto the image's top-left cell,
/// because the grid is the only thing that keeps an association on its text:
/// `alacritty_terminal` exposes no absolute line number and no scroll
/// callback, so a side table keyed by line drifts the first time output
/// scrolls. A cell carried into history and back, or rewrapped by a resize,
/// carries its zerowidth chars with it — which is how the same crate keeps an
/// OSC 8 hyperlink on its text.
const ANCHOR: u32 = 0xF_0000;
/// Plane 15 ends at `U+FFFFD`, which is the ceiling on live placements.
const ANCHOR_MAX: u32 = 0xF_FFFD - ANCHOR;

/// The codepoint written beside an anchor, plus which of the placement's
/// columns the cell is.
///
/// Every cell of a placement's top row carries the pair, so text over the
/// image's left edge still leaves cells that know where that edge was. Plane
/// 16, which keeps [`ANCHOR`]'s own range whole.
const ANCHOR_COLUMN: u32 = 0x10_0000;
/// Columns of a top row that carry one.
const ANCHOR_COLUMN_MAX: u32 = 256;

/// What a placement holds that the grid cannot: its size, and which image it
/// shows. Its *position* is the anchored cell.
#[derive(Debug, Clone, Copy)]
struct Placed {
    image: u32,
    /// The client's placement id, zero when it named none.
    placement: u32,
    cols: u16,
    rows: u16,
    frame: Frame,
    source: Source,
    z: i32,
}

/// A virtual placement: the box of `cols` by `rows` cells its placeholders
/// tile, and where the picture sits in it.
#[derive(Debug, Clone, Copy)]
struct Virtual {
    cols: u16,
    rows: u16,
    frame: Frame,
    source: Source,
    z: i32,
}

/// A placement positioned off another one: `offset` cells from its parent's
/// top-left cell.
#[derive(Debug, Clone, Copy)]
struct Relative {
    parent: (u32, u32),
    offset: (i32, i32),
    cols: u16,
    rows: u16,
    frame: Frame,
    source: Source,
    z: i32,
}

/// Placements found on the grid, each with what names it.
type Found<K> = Vec<(K, Placement)>;

/// What [`Emulator::locate`] found.
struct Located {
    /// Anchored placements, by anchor id.
    anchored: Found<u32>,
    /// Slices of virtual placements, by the placement each shows.
    pieces: Found<(u32, u32)>,
    /// Relative placements, by image and placement id.
    relatives: Found<(u32, u32)>,
}

/// The longest chain of relative placements, counting the one being made.
const MAX_RELATIVE_DEPTH: usize = 8;

/// Cursor position in viewport coordinates (row 0 = top of the visible grid).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CursorSnapshot {
    pub row: usize,
    pub col: usize,
}

/// Which kitty keyboard protocol enhancements the running program turned on.
///
/// `alacritty_terminal` keeps the mode stack, the alternate-screen swap and
/// the `CSI ? u` query reply; this is that state read back out for
/// [`crate::view::keystroke_bytes`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct KeyboardMode {
    /// Flag 1: `Esc` and the control and alt combos take a form of their own.
    pub disambiguate: bool,
    /// Flag 2: presses, repeats and releases are told apart.
    pub event_types: bool,
    /// Flag 4: a report carries the key the layout would have produced.
    pub alternate_keys: bool,
    /// Flag 8: every key is an escape code, printable or not.
    pub all_as_escapes: bool,
    /// Flag 16: a report carries the text the key produced.
    pub associated_text: bool,
    /// DECCKM, which moves the unmodified arrows and home/end to SS3.
    pub app_cursor: bool,
}

impl KeyboardMode {
    /// Whether any enhancement is on, which is what takes a key off its legacy
    /// encoding.
    pub fn enhanced(&self) -> bool {
        self.disambiguate || self.event_types || self.all_as_escapes
    }
}

/// Which pointer events the running program asked to be told about.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MouseTracking {
    /// The pointer is the user's: selection and scrollback.
    #[default]
    Off,
    /// `1000`: presses and releases.
    Click,
    /// `1002`: and motion while a button is held.
    Drag,
    /// `1003`: and motion with no button held.
    Motion,
}

/// How to report the pointer to the running program.
///
/// Handed to [`crate::view::mouse_bytes`], which is what turns a pointer event
/// into the bytes the program is waiting for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct MouseMode {
    pub tracking: MouseTracking,
    /// `1006`: SGR coordinates, which carry a grid past 223 columns.
    pub sgr: bool,
    /// `1005`: UTF-8 coordinates.
    pub utf8: bool,
    /// `1007`: a wheel tick on the alternate screen sends arrow keys.
    pub alternate_scroll: bool,
    /// Whether the alternate screen is up.
    pub alt_screen: bool,
    /// DECCKM, which decides whether those arrow keys are CSI or SS3.
    pub app_cursor: bool,
}

/// How long a [render hold](Emulator::render_hold) may last before the host
/// releases it. Matches the ceiling `vte` puts on its own buffering.
pub const HOLD_TIMEOUT: Duration = Duration::from_millis(150);

/// Turns off `vte`'s synchronized-update buffering.
///
/// `Processor::advance` diverts bytes into a buffer while `pending_timeout` is
/// true and replays them at ESU. Reporting no pending timeout keeps every byte
/// on the one path, so text and graphics reach the grid in the order the
/// program wrote them.
#[derive(Debug, Default)]
pub struct NoSync;

impl Timeout for NoSync {
    fn set_timeout(&mut self, _: Duration) {}

    fn clear_timeout(&mut self) {}

    fn pending_timeout(&self) -> bool {
        false
    }
}

/// The grid as it stood when a render hold began.
///
/// Nothing after the sequence that began the hold had been folded in yet, so
/// this is the frame the program means to leave on screen.
struct Held {
    lines: Vec<Vec<CellSnapshot>>,
    /// The scrollback offset the rows were read at, which is what maps a row
    /// back to a grid line when the live selection is stamped over them.
    offset: usize,
    cursor: Option<CursorSnapshot>,
    placements: Vec<Placement>,
}

/// Captures `Term` callbacks. Interior-mutable because `EventListener::send_event`
/// takes `&self`; single-threaded (the emulator lives inside a gpui entity).
#[derive(Default, Clone)]
struct EventCapture {
    events: Rc<RefCell<Vec<Event>>>,
}

impl EventListener for EventCapture {
    fn send_event(&self, event: Event) {
        self.events.borrow_mut().push(event);
    }
}

/// The emulator: a pure fold of PTY bytes into a renderable grid.
pub struct Emulator {
    term: Term<EventCapture>,
    parser: Processor<NoSync>,
    capture: EventCapture,
    title: Option<String>,
    directory: Option<PathBuf>,
    bell: bool,
    /// Splits graphics commands off the stream ahead of the parser. Held
    /// across feeds: a pty read ends wherever the kernel filled the buffer,
    /// which is as likely to be inside an escape as anywhere else.
    scanner: Scanner,
    graphics: kitty::Store,
    /// Live placements by anchor id. The grid holds where each one is; this
    /// holds what it is.
    placed: std::collections::HashMap<u32, Placed>,
    next_anchor: u32,
    /// Virtual placements (`U=1`) by image and placement id. They sit nowhere
    /// on the grid: placeholder cells name them.
    virtuals: std::collections::HashMap<(u32, u32), Virtual>,
    /// Relative placements by image and placement id. They sit nowhere on
    /// the grid either: each frame finds them from their parent.
    relatives: std::collections::HashMap<(u32, u32), Relative>,
    /// Whether DA1 is answered with sixel support in it.
    advertise_sixel: bool,
    /// An iTerm2 file arriving in parts: its arguments, and its bytes so far.
    multipart: Option<(Vec<u8>, Vec<u8>)>,
    /// One cell in pixels, which is what turns an image's pixel size into the
    /// rows it covers. The view measures it from the font every frame and the
    /// host hands it over; until then an image has no size the grid can use.
    cell: Option<(f32, f32)>,
    /// The frame served while a render hold is on.
    held: Option<Held>,
}

impl Emulator {
    pub fn new(cols: u16, rows: u16) -> Self {
        let capture = EventCapture::default();
        let config = Config {
            scrolling_history: SCROLLBACK_LINES,
            // Answers the `CSI ? u` query and keeps the mode stack. A program
            // reads the answer to decide whether to use the protocol at all,
            // so this and the encoder in `view` are one feature.
            kitty_keyboard: true,
            ..Config::default()
        };
        let term = Term::new(config, &GridSize::new(cols, rows), capture.clone());
        Self {
            term,
            parser: Processor::new(),
            capture,
            title: None,
            directory: None,
            bell: false,
            scanner: Scanner::new(),
            graphics: kitty::Store::new(),
            placed: std::collections::HashMap::new(),
            next_anchor: 0,
            virtuals: std::collections::HashMap::new(),
            relatives: std::collections::HashMap::new(),
            multipart: None,
            advertise_sixel: false,
            cell: None,
            held: None,
        }
    }

    /// The pixel size of one cell, measured by the view. An image arriving
    /// before the first frame has nothing to size itself against and is stored
    /// without being placed.
    pub fn set_cell_size(&mut self, width: f32, height: f32) {
        if width > 0.0 && height > 0.0 {
            self.cell = Some((width, height));
        }
    }

    /// Advance the state machine over decoded PTY output. Returns bytes the
    /// terminal wants written back to the PTY (DSR/DA query responses,
    /// graphics acknowledgements).
    ///
    /// Replies leave in the order their queries arrived. A program probing
    /// with several queries and a `CSI c` behind them reads the DA answer as
    /// the end of the replies it will get.
    ///
    /// Mode 2026 is acted on where it sits too: the frame the program wants
    /// left on screen is the grid as it stands when the mode goes on.
    ///
    /// `CSI 14 t` and `CSI 16 t` go unanswered until
    /// [`Emulator::set_cell_size`] has been called.
    pub fn feed(&mut self, bytes: &[u8]) -> Vec<u8> {
        let mut responses = Vec::new();
        for segment in self.scanner.feed(bytes) {
            match segment {
                Segment::Text(text) => {
                    self.parser.advance(&mut self.term, text);
                    self.drain_events(&mut responses);
                }
                Segment::Graphics(command) => {
                    let (effect, reply) = self.graphics.apply(command);
                    let reply = match effect {
                        Some(kitty::Effect::Display(display)) => match self.place(display) {
                            Ok(()) => reply,
                            // The store said yes to what it could see; the
                            // placement is refused on what only the grid knows.
                            Err(error) => (display.quiet < 2).then(|| kitty::Reply {
                                id: display.image,
                                number: reply.as_ref().map_or(0, |reply| reply.number),
                                placement: display.placement,
                                error: Some(error),
                            }),
                        },
                        Some(kitty::Effect::Delete(delete)) => {
                            self.delete(delete);
                            reply
                        }
                        None => reply,
                    };
                    if let Some(reply) = reply {
                        responses.extend(reply.bytes());
                    }
                }
                Segment::Sync(true) => self.hold(),
                Segment::Sync(false) => self.held = None,
                Segment::Sixel(data) => self.sixel(&data),
                Segment::Iterm(command) => self.iterm(command),
                Segment::Directory(path) => self.directory = Some(path),
                Segment::CellSizeQuery => {
                    if let Some(size) = self.window_size() {
                        let reply = format!("\x1b[6;{};{}t", size.cell_height, size.cell_width);
                        responses.extend_from_slice(reply.as_bytes());
                    }
                }
            }
        }
        responses
    }

    /// Act on what `Term` raised while the parser ran.
    fn drain_events(&mut self, responses: &mut Vec<u8>) {
        let window = self.window_size();
        for event in self.capture.events.borrow_mut().drain(..) {
            match event {
                // `Term` answers DA1 as a VT102.
                Event::PtyWrite(text) if self.advertise_sixel && text == "\x1b[?6c" => {
                    responses.extend_from_slice(b"\x1b[?62;4;22c")
                }
                Event::PtyWrite(text) => responses.extend_from_slice(text.as_bytes()),
                Event::TextAreaSizeRequest(reply) => {
                    if let Some(window) = window {
                        responses.extend_from_slice(reply(window).as_bytes());
                    }
                }
                Event::Title(title) => self.title = Some(title),
                Event::ResetTitle => self.title = None,
                Event::Bell => self.bell = true,
                _ => {}
            }
        }
    }

    /// The grid and its cell in whole pixels, which is what the size reports
    /// carry. `None` until the view has measured a cell.
    fn window_size(&self) -> Option<WindowSize> {
        let (width, height) = self.cell?;
        Some(WindowSize {
            num_lines: self.term.screen_lines() as u16,
            num_cols: self.term.columns() as u16,
            cell_width: width.round() as u16,
            cell_height: height.round() as u16,
        })
    }

    pub fn resize(&mut self, cols: u16, rows: u16) {
        self.held = None;
        self.term.resize(GridSize::new(cols, rows));
    }

    // ---- render hold ----

    /// Whether the running program has asked for the screen to stop updating
    /// (mode 2026). While this is true, [`Emulator::lines`], [`Emulator::line`],
    /// [`Emulator::cursor`] and [`Emulator::placements`] serve the frame the
    /// grid held when the hold began; everything else stays live.
    ///
    /// Nothing here ends a hold on its own: the emulator has no clock. A host
    /// that does not release a stuck hold within [`HOLD_TIMEOUT`] shows an
    /// unfinished frame for as long as the program leaves the mode on.
    /// [`Emulator::resize`] ends one.
    pub fn render_hold(&self) -> bool {
        self.held.is_some()
    }

    /// End a hold the program never ended.
    pub fn release_hold(&mut self) {
        self.held = None;
    }

    /// Capture the frame a hold begins on. A hold already on stays on its own
    /// frame: setting the mode twice is one hold.
    fn hold(&mut self) {
        if self.held.is_some() {
            return;
        }
        self.held = Some(Held {
            lines: self.live_lines(),
            offset: self.display_offset(),
            cursor: self.live_cursor(),
            placements: self.live_placements(),
        });
    }

    pub fn cols(&self) -> usize {
        self.term.columns()
    }

    pub fn rows(&self) -> usize {
        self.term.screen_lines()
    }

    /// OSC title, if the running program set one.
    pub fn title(&self) -> Option<&str> {
        self.title.as_deref()
    }

    /// The working directory the shell last reported through `OSC 7` or
    /// `OSC 9 ; 9`. `None` until one arrives; a shell that emits neither never
    /// sets it.
    pub fn directory(&self) -> Option<&Path> {
        self.directory.as_deref()
    }

    /// True once a BEL arrived; reading clears it.
    pub fn take_bell(&mut self) -> bool {
        std::mem::take(&mut self.bell)
    }

    /// Arrow keys should send SS3 (`ESC O A`) instead of CSI.
    pub fn app_cursor_mode(&self) -> bool {
        self.term.mode().contains(TermMode::APP_CURSOR)
    }

    /// What the running program wants done with the keyboard.
    pub fn keyboard_mode(&self) -> KeyboardMode {
        let mode = self.term.mode();
        KeyboardMode {
            disambiguate: mode.contains(TermMode::DISAMBIGUATE_ESC_CODES),
            event_types: mode.contains(TermMode::REPORT_EVENT_TYPES),
            alternate_keys: mode.contains(TermMode::REPORT_ALTERNATE_KEYS),
            all_as_escapes: mode.contains(TermMode::REPORT_ALL_KEYS_AS_ESC),
            associated_text: mode.contains(TermMode::REPORT_ASSOCIATED_TEXT),
            app_cursor: mode.contains(TermMode::APP_CURSOR),
        }
    }

    /// What the running program wants done with the pointer.
    pub fn mouse_mode(&self) -> MouseMode {
        let mode = self.term.mode();
        // `Term` clears the other two whenever it sets one of these, so the
        // order here only decides what a program setting several would get.
        let tracking = if mode.contains(TermMode::MOUSE_MOTION) {
            MouseTracking::Motion
        } else if mode.contains(TermMode::MOUSE_DRAG) {
            MouseTracking::Drag
        } else if mode.contains(TermMode::MOUSE_REPORT_CLICK) {
            MouseTracking::Click
        } else {
            MouseTracking::Off
        };
        MouseMode {
            tracking,
            sgr: mode.contains(TermMode::SGR_MOUSE),
            utf8: mode.contains(TermMode::UTF8_MOUSE),
            alternate_scroll: mode.contains(TermMode::ALTERNATE_SCROLL),
            alt_screen: mode.contains(TermMode::ALT_SCREEN),
            app_cursor: mode.contains(TermMode::APP_CURSOR),
        }
    }

    /// Pastes should be wrapped in `ESC [200~` / `ESC [201~`.
    pub fn bracketed_paste_mode(&self) -> bool {
        self.term.mode().contains(TermMode::BRACKETED_PASTE)
    }

    /// Lines scrolled back into history (0 = pinned to the live bottom).
    pub fn display_offset(&self) -> usize {
        self.term.grid().display_offset()
    }

    /// Lines available above the viewport.
    pub fn history_lines(&self) -> usize {
        self.term.grid().history_size()
    }

    /// Scroll the view: positive = up into history, negative = toward live.
    pub fn scroll(&mut self, delta: i32) {
        self.term.scroll_display(Scroll::Delta(delta));
    }

    pub fn scroll_to_bottom(&mut self) {
        self.term.scroll_display(Scroll::Bottom);
    }
}

impl std::fmt::Debug for Emulator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Emulator")
            .field("cols", &self.cols())
            .field("rows", &self.rows())
            .field("display_offset", &self.display_offset())
            .finish()
    }
}
