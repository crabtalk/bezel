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

    /// The images the client has sent, by id.
    pub fn graphics(&self) -> &kitty::Store {
        &self.graphics
    }

    /// Answer DA1 as a VT220 with sixel graphics (`CSI ? 62 ; 4 ; 22 c`)
    /// rather than as a VT102 (`CSI ? 6 c`). Off until a host turns it on.
    /// Programs read attribute 4 in that answer to decide whether to send
    /// sixel images; they are shown either way.
    pub fn set_advertise_sixel(&mut self, advertise: bool) {
        self.advertise_sixel = advertise;
    }

    /// See [`kitty::Store::set_local_media`].
    pub fn set_local_media(&mut self, allow: bool) {
        self.graphics.set_local_media(allow);
    }

    /// Put an image on the grid at the cursor, and move the cursor past it
    /// unless the command said not to.
    ///
    /// The rows it covers are reserved by feeding that many linefeeds through
    /// the parser rather than by moving the cursor directly: a linefeed at the
    /// bottom of the screen scrolls, which is what pushes the image's own
    /// anchors into history at the same moment its text would have gone.
    /// `C=1` reserves nothing, so the image covers whatever is drawn under it.
    ///
    /// A placement replaces the one the image already has under the same
    /// placement id, zero included: an image placed with no `p` has at most
    /// one placement.
    fn place(&mut self, display: kitty::Display) -> Result<(), &'static str> {
        let image = display.image;
        let key = (image, display.placement);
        if display.parent_image != 0 {
            return self.relate(display);
        }
        let Some((cols, rows, frame, source)) = self.extent(&display) else {
            return Ok(());
        };
        self.relatives.remove(&key);
        if display.unicode {
            let virtual_ = Virtual {
                cols: cols.max(1.0).min(u16::MAX as f32) as u16,
                rows: rows.max(1.0).min(u16::MAX as f32) as u16,
                frame,
                source,
                z: display.z,
            };
            self.virtuals.insert(key, virtual_);
            return Ok(());
        }
        let cols = (cols as usize).clamp(1, self.cols()) as u16;
        let rows = (rows as usize).clamp(1, self.rows()) as u16;

        self.anchor(display, cols, rows, frame, source);
        Ok(())
    }

    /// Show a sixel image at the cursor, the way `a=T` shows one.
    fn sixel(&mut self, data: &[u8]) {
        let Some(rgba) = crate::sixel::decode(data) else {
            return;
        };
        let image = kitty::Image::still(kitty::Format::Rgba, rgba.width, rgba.height, rgba.bytes);
        let id = self.graphics.hold(image);
        let _ = self.place(kitty::Display::at_cursor(id));
    }

    /// An iTerm2 file command: a whole file, or a part of one.
    fn iterm(&mut self, command: Iterm) {
        match command {
            Iterm::File { args, payload } => {
                self.iterm_show(&args, &kitty::decode(&payload));
            }
            Iterm::Begin { args } => self.multipart = Some((args, Vec::new())),
            Iterm::Part(payload) => {
                let Some((_, bytes)) = &mut self.multipart else {
                    return;
                };
                bytes.extend(kitty::decode(&payload));
                if bytes.len() > crate::iterm::MAX_FILE {
                    self.multipart = None;
                }
            }
            Iterm::End => {
                if let Some((args, bytes)) = self.multipart.take() {
                    self.iterm_show(&args, &bytes);
                }
            }
        }
    }

    /// Show an iTerm2 file at the cursor, if it is an inline image.
    fn iterm_show(&mut self, args: &[u8], bytes: &[u8]) {
        let args = crate::iterm::Args::parse(args);
        if !args.inline || bytes.len() > crate::iterm::MAX_FILE {
            return;
        }
        let Some(image) = crate::iterm::image(bytes) else {
            return;
        };
        let id = self.graphics.hold(image);
        let mut display = kitty::Display::at_cursor(id);
        if let Some((cell_w, cell_h)) = self.cell {
            display.columns = args.width.cells(cell_w, self.cols());
            display.rows = args.height.cells(cell_h, self.rows());
        }
        display.stretch = !args.preserve_aspect;
        let _ = self.place(display);
    }

    /// Hang a placement off the parent its `P` and `Q` name. The cursor stays
    /// where it is, whatever `C` says.
    fn relate(&mut self, display: kitty::Display) -> Result<(), &'static str> {
        let key = (display.image, display.placement);
        let parent = (display.parent_image, display.parent_placement);
        if display.unicode {
            return Err("EINVAL:a virtual placement cannot be relative");
        }
        if !self.exists(parent) {
            return Err("ENOPARENT");
        }
        let mut depth = 1;
        let mut at = parent;
        loop {
            if at == key {
                return Err("ECYCLE");
            }
            match self.relatives.get(&at) {
                Some(relative) => {
                    depth += 1;
                    at = relative.parent;
                }
                None => break,
            }
        }
        if depth > MAX_RELATIVE_DEPTH {
            return Err("ETOODEEP");
        }
        let Some((cols, rows, frame, source)) = self.extent(&display) else {
            return Ok(());
        };
        self.placed
            .retain(|_, placed| (placed.image, placed.placement) != key);
        self.virtuals.remove(&key);
        self.relatives.insert(
            key,
            Relative {
                parent,
                offset: (display.parent_offset_x, display.parent_offset_y),
                cols: cols.clamp(1.0, u16::MAX as f32) as u16,
                rows: rows.clamp(1.0, u16::MAX as f32) as u16,
                frame,
                source,
                z: display.z,
            },
        );
        Ok(())
    }

    /// Whether any placement, of any kind, goes by `key`.
    fn exists(&self, key: (u32, u32)) -> bool {
        self.placed
            .values()
            .any(|placed| (placed.image, placed.placement) == key)
            || self.virtuals.contains_key(&key)
            || self.relatives.contains_key(&key)
    }

    /// Whether any placement, of any kind, shows `image`.
    fn shown(&self, image: u32) -> bool {
        self.placed.values().any(|placed| placed.image == image)
            || self.virtuals.keys().any(|&(held, _)| held == image)
            || self.relatives.keys().any(|&(held, _)| held == image)
    }

    /// The cells a display covers, unclamped, with its frame and source.
    /// `None` until a cell has been measured, and for an image with nothing
    /// to show.
    fn extent(&self, display: &kitty::Display) -> Option<(f32, f32, Frame, Source)> {
        let (cell_w, cell_h) = self.cell?;
        let (width, height) = self
            .graphics
            .get(display.image)
            .and_then(kitty::Image::size)?;
        let x = display.source_x.min(width);
        let y = display.source_y.min(height);
        let source = Source {
            x,
            y,
            width: match display.source_width {
                0 => width - x,
                w => w.min(width - x),
            },
            height: match display.source_height {
                0 => height - y,
                h => h.min(height - y),
            },
        };
        if source.width == 0 || source.height == 0 {
            return None;
        }
        let (source_w, source_h) = (source.width as f32, source.height as f32);
        let offset_x = (display.offset_x as f32).min(cell_w - 1.0).max(0.0);
        let offset_y = (display.offset_y as f32).min(cell_h - 1.0).max(0.0);
        // The frame in pixels, and the cells it covers. Unscaled without `c`
        // and `r`; one of them scales the other by the source's aspect; both
        // fit the source inside the box they make, centred. The offset counts
        // toward the cells only when neither is given.
        let (cols, rows, frame) = match (display.columns, display.rows) {
            (0, 0) => (
                ((offset_x + source_w) / cell_w).ceil(),
                ((offset_y + source_h) / cell_h).ceil(),
                (offset_x, offset_y, source_w, source_h),
            ),
            (c, 0) => {
                let w = c as f32 * cell_w;
                let h = w * source_h / source_w;
                (c as f32, (h / cell_h).ceil(), (offset_x, offset_y, w, h))
            }
            (0, r) => {
                let h = r as f32 * cell_h;
                let w = h * source_w / source_h;
                ((w / cell_w).ceil(), r as f32, (offset_x, offset_y, w, h))
            }
            (c, r) if display.stretch => {
                let (w, h) = (c as f32 * cell_w, r as f32 * cell_h);
                (c as f32, r as f32, (offset_x, offset_y, w, h))
            }
            (c, r) => {
                let (box_w, box_h) = (c as f32 * cell_w, r as f32 * cell_h);
                let scale = (box_w / source_w).min(box_h / source_h);
                let (w, h) = (source_w * scale, source_h * scale);
                let x = offset_x + (box_w - w) / 2.0;
                let y = offset_y + (box_h - h) / 2.0;
                (c as f32, r as f32, (x, y, w, h))
            }
        };
        let frame = Frame {
            x: frame.0 / cell_w,
            y: frame.1 / cell_h,
            width: frame.2 / cell_w,
            height: frame.3 / cell_h,
        };
        Some((cols, rows, frame, source))
    }

    /// Write a placement's anchors at the cursor, and move the cursor past it
    /// unless the display said not to.
    fn anchor(
        &mut self,
        display: kitty::Display,
        cols: u16,
        rows: u16,
        frame: Frame,
        source: Source,
    ) {
        let image = display.image;
        self.placed
            .retain(|_, placed| placed.image != image || placed.placement != display.placement);
        let anchor = self.next_anchor % ANCHOR_MAX;
        self.next_anchor = anchor.wrapping_add(1);
        // The id is reused once the ring wraps, so whatever wore it last stops
        // being a placement before the new one starts.
        self.placed.remove(&anchor);

        let cursor = self.term.grid().cursor.point;
        let width = (cols as usize)
            .min(ANCHOR_COLUMN_MAX as usize)
            .min(self.cols() - cursor.column.0);
        for offset in 0..width {
            let cell = &mut self.term.grid_mut()[cursor.line][Column(cursor.column.0 + offset)];
            // A cell only ever gains zerowidth chars, so one anchored again
            // without being written to in between would collect a pair per
            // redraw. Rebuilt from the marks still worth keeping — the pairs
            // of placements still live — which takes this cell's underline
            // color and hyperlink with it.
            if cell
                .zerowidth()
                .is_some_and(|marks| marks.iter().any(|&mark| is_anchor(mark)))
            {
                let marks = cell.zerowidth().unwrap_or(&[]);
                let mut kept: Vec<char> = marks
                    .iter()
                    .copied()
                    .filter(|&mark| !is_anchor(mark))
                    .collect();
                for (live, column) in anchor_pairs(marks) {
                    if !self.placed.contains_key(&live) {
                        continue;
                    }
                    kept.extend(char::from_u32(ANCHOR + live));
                    kept.extend(
                        column.and_then(|column| char::from_u32(ANCHOR_COLUMN + column as u32)),
                    );
                }
                cell.extra = None;
                for mark in kept {
                    cell.push_zerowidth(mark);
                }
            }
            for mark in [ANCHOR + anchor, ANCHOR_COLUMN + offset as u32] {
                if let Some(mark) = char::from_u32(mark) {
                    cell.push_zerowidth(mark);
                }
            }
        }
        self.placed.insert(
            anchor,
            Placed {
                image,
                placement: display.placement,
                cols,
                rows,
                frame,
                source,
                z: display.z,
            },
        );
        if display.cursor_movement == kitty::CursorMovement::After {
            for _ in 0..rows {
                self.parser.advance(&mut self.term, b"\n");
            }
        }
    }

    /// Every image on the visible grid, found by the anchors the cells carry.
    ///
    /// Walked per frame rather than cached: an anchor moves with its cell, and
    /// nothing tells us when. Each placement is taken from the first of its
    /// anchors the walk reaches, whose own column is what puts the image's
    /// left edge back; a placement with no anchor left is one whose whole top
    /// row was overwritten, which is what clearing the screen does to it.
    pub fn placements(&self) -> Vec<Placement> {
        match &self.held {
            Some(held) => held.placements.clone(),
            None => self.live_placements(),
        }
    }

    fn live_placements(&self) -> Vec<Placement> {
        let located = self.locate(self.display_offset() as i32);
        let mut out: Vec<Placement> = located.anchored.into_iter().map(|(_, p)| p).collect();
        out.extend(located.pieces.into_iter().map(|(_, p)| p));
        out.extend(located.relatives.into_iter().map(|(_, p)| p));
        out
    }

    /// Every placement on the rows `offset` lines above the bottom of the
    /// screen, each with what names it.
    ///
    /// A relative placement sits where its parent is found on those rows, so
    /// one whose parent is off them is not found either. A virtual parent is
    /// at the least row and least column of the placeholder cells showing it.
    fn locate(&self, offset: i32) -> Located {
        let (anchored, pieces) = self.scan(offset);
        let mut relatives = Vec::new();
        if !self.relatives.is_empty() {
            let mut at: std::collections::HashMap<(u32, u32), (i64, i64)> =
                std::collections::HashMap::new();
            for (anchor, p) in &anchored {
                if let Some(placed) = self.placed.get(anchor) {
                    at.insert(
                        (placed.image, placed.placement),
                        (p.row as i64, p.col as i64),
                    );
                }
            }
            for (key, p) in &pieces {
                let spot = at.entry(*key).or_insert((p.row as i64, p.col as i64));
                *spot = (spot.0.min(p.row as i64), spot.1.min(p.col as i64));
            }
            for (&key, relative) in &self.relatives {
                let Some((row, col)) = self.relative_spot(key, &at) else {
                    continue;
                };
                if row < 0 || col < 0 || row >= self.rows() as i64 || col >= self.cols() as i64 {
                    continue;
                }
                relatives.push((
                    key,
                    Placement {
                        row: row as usize,
                        col: col as usize,
                        cols: relative.cols,
                        rows: relative.rows,
                        image: key.0,
                        frame: relative.frame,
                        source: relative.source,
                        z: relative.z,
                    },
                ));
            }
        }
        Located {
            anchored,
            pieces,
            relatives,
        }
    }

    /// Where a relative placement's top-left cell is, walking its chain up
    /// to a parent found on the grid.
    fn relative_spot(
        &self,
        key: (u32, u32),
        at: &std::collections::HashMap<(u32, u32), (i64, i64)>,
    ) -> Option<(i64, i64)> {
        let mut row = 0;
        let mut col = 0;
        let mut key = key;
        for _ in 0..=MAX_RELATIVE_DEPTH {
            let Some(relative) = self.relatives.get(&key) else {
                return at.get(&key).map(|&(r, c)| (r + row, c + col));
            };
            row += relative.offset.1 as i64;
            col += relative.offset.0 as i64;
            key = relative.parent;
        }
        None
    }

    /// The placements on the rows `offset` lines above the bottom of the
    /// screen, in one walk of their cells: anchored ones by anchor id, and the
    /// slices of virtual placements that placeholder cells show, one per run
    /// of cells that continue each other along a row. Zero is the screen the
    /// cursor moves on.
    fn scan(&self, offset: i32) -> (Found<u32>, Found<(u32, u32)>) {
        let mut anchored: Vec<(u32, Placement)> = Vec::new();
        let mut seen = std::collections::HashSet::new();
        let mut pieces: Vec<((u32, u32), Placement)> = Vec::new();
        let grid = self.term.grid();
        for row in 0..self.rows() {
            let line = Line(row as i32 - offset);
            let mut decoder =
                (!self.virtuals.is_empty()).then(crate::placeholder::RowDecoder::default);
            // The run being grown: the key it was resolved to, the slot of its
            // last cell, and its index in `pieces`.
            let mut run: Option<((u32, u32), crate::placeholder::Slot, usize)> = None;
            for col in 0..self.cols() {
                let cell = &grid[line][Column(col)];
                let marks = cell.zerowidth().unwrap_or(&[]);
                for (anchor, within) in anchor_pairs(marks) {
                    let Some(placed) = self.placed.get(&anchor) else {
                        continue;
                    };
                    if !seen.insert(anchor) {
                        continue;
                    }
                    // A cell whose column mark was lost still holds the image;
                    // it can only say the left edge is here.
                    let within = within.unwrap_or(0);
                    anchored.push((
                        anchor,
                        Placement {
                            row,
                            col: col.saturating_sub(within),
                            cols: placed.cols,
                            rows: placed.rows,
                            image: placed.image,
                            frame: placed.frame,
                            source: placed.source,
                            z: placed.z,
                        },
                    ));
                }
                let Some(decoder) = decoder.as_mut() else {
                    continue;
                };
                let shown = decoder
                    .cell(cell)
                    .and_then(|slot| self.shown_by(slot).map(|(key, v)| (slot, key, v)));
                let Some((slot, key, virtual_)) = shown else {
                    run = None;
                    continue;
                };
                if let Some((run_key, last, at)) = &mut run
                    && *run_key == key
                    && last.row == slot.row
                    && last.col + 1 == slot.col
                {
                    pieces[*at].1.cols += 1;
                    *last = slot;
                    continue;
                }
                run = Some((key, slot, pieces.len()));
                pieces.push((
                    key,
                    Placement {
                        row,
                        col,
                        cols: 1,
                        rows: 1,
                        image: key.0,
                        frame: Frame {
                            x: virtual_.frame.x - slot.col as f32,
                            y: virtual_.frame.y - slot.row as f32,
                            ..virtual_.frame
                        },
                        source: virtual_.source,
                        z: virtual_.z,
                    },
                ));
            }
        }
        (anchored, pieces)
    }

    /// The virtual placement a placeholder cell shows, and its key. A cell
    /// naming no placement id shows the image's virtual placement with the
    /// lowest id. A cell outside its placement's box shows nothing.
    fn shown_by(&self, slot: crate::placeholder::Slot) -> Option<((u32, u32), &Virtual)> {
        let key = match slot.placement {
            0 => self
                .virtuals
                .keys()
                .filter(|(image, _)| *image == slot.image)
                .min()
                .copied()?,
            placement => (slot.image, placement),
        };
        let virtual_ = self.virtuals.get(&key)?;
        (slot.row < virtual_.rows as u32 && slot.col < virtual_.cols as u32)
            .then_some((key, virtual_))
    }

    /// Take the placements a delete names off the grid, and with an
    /// upper-case one, free the data of each image it leaves with none.
    ///
    /// The ones named by position — every target but an image id, a range of
    /// them and a z-index — are looked for on the screen alone, not in
    /// history.
    fn delete(&mut self, delete: kitty::Delete) {
        use kitty::Target;
        if delete.target == Target::All && delete.free {
            self.placed.clear();
            self.relatives.clear();
            self.graphics.clear();
            return;
        }
        let covers = |p: &Placement, col: u32, row: u32| {
            let (col, row) = (col as usize, row as usize);
            (p.col..p.col + p.cols as usize).contains(&col)
                && (p.row..p.row + p.rows as usize).contains(&row)
        };
        // Which keys a delete names: an image id and placement, a range of
        // ids, or a z-index. `None` for the deletes that name positions.
        let names = |image: u32, placement: u32, z: i32| match delete.target {
            Target::Image { id, placement: p } => Some(image == id && (p == 0 || placement == p)),
            Target::Range(low, high) => Some((low..=high).contains(&image)),
            Target::Z(want) => Some(z == want),
            _ => None,
        };
        let by_name = matches!(
            delete.target,
            Target::Image { .. } | Target::Range(..) | Target::Z(_)
        );
        let mut anchors: Vec<u32> = Vec::new();
        let mut keys: Vec<(u32, u32)> = Vec::new();
        if by_name {
            anchors.extend(
                self.placed
                    .iter()
                    .filter(|(_, p)| names(p.image, p.placement, p.z) == Some(true))
                    .map(|(&anchor, _)| anchor),
            );
            keys.extend(
                self.relatives
                    .iter()
                    .filter(|(key, r)| names(key.0, key.1, r.z) == Some(true))
                    .map(|(&key, _)| key),
            );
        } else {
            let cursor = self.term.grid().cursor.point;
            let hit = |p: &Placement| match delete.target {
                Target::Cursor => covers(p, cursor.column.0 as u32, cursor.line.0 as u32),
                Target::Cell { col, row, z } => covers(p, col, row) && z.is_none_or(|z| p.z == z),
                Target::Column(col) => (p.col..p.col + p.cols as usize).contains(&(col as usize)),
                Target::Row(row) => (p.row..p.row + p.rows as usize).contains(&(row as usize)),
                _ => true,
            };
            let located = self.locate(0);
            anchors.extend(
                located
                    .anchored
                    .iter()
                    .filter(|(_, p)| hit(p))
                    .map(|(a, _)| *a),
            );
            keys.extend(
                located
                    .relatives
                    .iter()
                    .filter(|(_, p)| hit(p))
                    .map(|(k, _)| *k),
            );
        }
        let mut touched: Vec<u32> = Vec::new();
        // A virtual placement has no position, so only the deletes that name
        // images reach one — and a z-index is not one of those.
        if !matches!(delete.target, Target::Z(_)) && by_name {
            self.virtuals.retain(|&(image, placement), _| {
                let hit = names(image, placement, 0) == Some(true);
                if hit {
                    touched.push(image);
                }
                !hit
            });
        }
        touched.extend(
            anchors
                .iter()
                .filter_map(|anchor| self.placed.remove(anchor))
                .map(|placed| placed.image),
        );
        for key in keys {
            if self.relatives.remove(&key).is_some() {
                touched.push(key.0);
            }
        }
        // A relative placement goes with its parent, and an image left with
        // no placement by that goes too, whatever the case of `d`.
        loop {
            let orphans: Vec<(u32, u32)> = self
                .relatives
                .iter()
                .filter(|(_, relative)| !self.exists(relative.parent))
                .map(|(&key, _)| key)
                .collect();
            if orphans.is_empty() {
                break;
            }
            for key in orphans {
                self.relatives.remove(&key);
                if !self.shown(key.0) {
                    self.graphics.remove(key.0);
                }
            }
        }
        if !delete.free {
            return;
        }
        if let Target::Image { id, .. } = delete.target {
            touched.push(id);
        }
        for image in touched {
            if !self.shown(image) {
                self.graphics.remove(image);
            }
        }
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

    // ---- selection ----
    //
    // `Term` owns the selection outright, which is what makes this cheap: it
    // rotates the anchors when output scrolls the grid and drops them on clear
    // and resize, so a selection tracks live output without any bookkeeping
    // here. The host supplies pointer positions; everything below is a thin
    // translation into grid coordinates.

    /// The grid point under a viewport cell (row 0 = top of the visible area).
    ///
    /// Viewport rows are what the pointer hits; grid lines are what a selection
    /// anchors to, and the two differ by the scrollback offset. Anchoring in
    /// grid space is what lets a selection stay on its text while the view
    /// scrolls out from under it.
    pub fn grid_point(&self, viewport_row: usize, col: usize) -> Point {
        Point::new(
            Line(viewport_row as i32 - self.display_offset() as i32),
            Column(col.min(self.cols().saturating_sub(1))),
        )
    }

    /// Begin a selection. `ty` picks the granularity: [`SelectionType::Simple`]
    /// for a drag, `Semantic` for a double-click word, `Lines` for a triple-
    /// click row.
    pub fn start_selection(&mut self, ty: SelectionType, point: Point, side: Side) {
        self.term.selection = Some(Selection::new(ty, point, side));
    }

    /// Extend the in-progress selection to `point`. No-op without one.
    pub fn update_selection(&mut self, point: Point, side: Side) {
        if let Some(selection) = self.term.selection.as_mut() {
            selection.update(point, side);
        }
    }

    pub fn clear_selection(&mut self) {
        self.term.selection = None;
    }

    /// The selected text, or `None` when there is no selection or it covers
    /// nothing (a click without a drag leaves an empty one behind).
    pub fn selection_text(&self) -> Option<String> {
        self.term
            .selection_to_string()
            // `selection_to_string` folds a cell's zerowidth chars into the
            // text, anchors included. Copying across an image would otherwise
            // paste a private-use character nobody can see.
            .map(|text| text.replace(|ch: char| is_anchor(ch), ""))
            .filter(|s| !s.is_empty())
    }

    /// Whether a non-empty selection is active — drives the copy action and
    /// the "clear it" branch on the next click.
    pub fn has_selection(&self) -> bool {
        self.selection_range().is_some()
    }

    fn selection_range(&self) -> Option<SelectionRange> {
        self.term
            .selection
            .as_ref()
            .and_then(|selection| selection.to_range(&self.term))
    }

    /// Snapshot one viewport row (0 = top) honoring the scrollback offset.
    pub fn line(&self, viewport_row: usize) -> Vec<CellSnapshot> {
        let selection = self.selection_range();
        let Some(held) = &self.held else {
            return self.line_inner(viewport_row, selection);
        };
        held.lines
            .get(viewport_row)
            .map(|cells| self.reselect(held, viewport_row, cells, selection))
            .unwrap_or_default()
    }

    /// A held row with `selected` recomputed against the live selection. A
    /// hold stops output, not the pointer.
    fn reselect(
        &self,
        held: &Held,
        viewport_row: usize,
        cells: &[CellSnapshot],
        selection: Option<SelectionRange>,
    ) -> Vec<CellSnapshot> {
        let line = Line(viewport_row as i32 - held.offset as i32);
        cells
            .iter()
            .enumerate()
            .map(|(col, cell)| CellSnapshot {
                selected: selection
                    .is_some_and(|range| range.contains(Point::new(line, Column(col)))),
                ..*cell
            })
            .collect()
    }

    /// The shared body of [`Self::line`], taking the selection range as an
    /// argument so [`Self::lines`] resolves it once per frame rather than once
    /// per row — `to_range` re-walks the grid for semantic and line selections.
    fn line_inner(
        &self,
        viewport_row: usize,
        selection: Option<SelectionRange>,
    ) -> Vec<CellSnapshot> {
        let offset = self.display_offset() as i32;
        let line = Line(viewport_row as i32 - offset);
        let grid = self.term.grid();
        let row = &grid[line];
        (0..self.cols())
            .map(|col| {
                let cell = &row[Column(col)];
                CellSnapshot {
                    // Stands for an image; the glyph itself is never drawn.
                    ch: match cell.c {
                        crate::placeholder::PLACEHOLDER => ' ',
                        ch => ch,
                    },
                    fg: map_color(cell.fg),
                    bg: map_color(cell.bg),
                    bold: cell.flags.intersects(Flags::BOLD),
                    dim: cell.flags.intersects(Flags::DIM),
                    italic: cell.flags.intersects(Flags::ITALIC),
                    underline: cell.flags.intersects(Flags::ALL_UNDERLINES),
                    inverse: cell.flags.intersects(Flags::INVERSE),
                    hidden: cell.flags.intersects(Flags::HIDDEN),
                    wide: cell.flags.intersects(Flags::WIDE_CHAR),
                    wide_spacer: cell
                        .flags
                        .intersects(Flags::WIDE_CHAR_SPACER | Flags::LEADING_WIDE_CHAR_SPACER),
                    selected: selection
                        .is_some_and(|range| range.contains(Point::new(line, Column(col)))),
                }
            })
            .collect()
    }

    /// All viewport rows, top to bottom.
    pub fn lines(&self) -> Vec<Vec<CellSnapshot>> {
        let selection = self.selection_range();
        let Some(held) = &self.held else {
            return self.live_lines();
        };
        held.lines
            .iter()
            .enumerate()
            .map(|(row, cells)| self.reselect(held, row, cells, selection))
            .collect()
    }

    fn live_lines(&self) -> Vec<Vec<CellSnapshot>> {
        let selection = self.selection_range();
        (0..self.rows())
            .map(|r| self.line_inner(r, selection))
            .collect()
    }

    /// Cursor in viewport coordinates; `None` when hidden or scrolled out.
    pub fn cursor(&self) -> Option<CursorSnapshot> {
        match &self.held {
            Some(held) => held.cursor,
            None => self.live_cursor(),
        }
    }

    fn live_cursor(&self) -> Option<CursorSnapshot> {
        let content = self.term.renderable_content();
        if content.cursor.shape == CursorShape::Hidden {
            return None;
        }
        let Point { line, column } = content.cursor.point;
        let row = line.0 + self.display_offset() as i32;
        if row < 0 || row >= self.rows() as i32 {
            return None;
        }
        Some(CursorSnapshot {
            row: row as usize,
            col: column.0,
        })
    }

    /// Test/diagnostic helper: a viewport row as trimmed text (wide-char
    /// spacers skipped).
    pub fn row_text(&self, viewport_row: usize) -> String {
        let mut text: String = self
            .line(viewport_row)
            .iter()
            .filter(|c| !c.wide_spacer)
            .map(|c| c.ch)
            .collect();
        while text.ends_with(' ') {
            text.pop();
        }
        text
    }
}

/// The anchors in a cell's zerowidth marks, each with the column mark written
/// straight after it.
fn anchor_pairs(marks: &[char]) -> impl Iterator<Item = (u32, Option<usize>)> + '_ {
    marks.iter().enumerate().filter_map(|(at, &mark)| {
        let anchor = (mark as u32).wrapping_sub(ANCHOR);
        if anchor >= ANCHOR_MAX {
            return None;
        }
        let column = marks.get(at + 1).and_then(|&next| {
            let index = (next as u32).wrapping_sub(ANCHOR_COLUMN);
            (index < ANCHOR_COLUMN_MAX).then_some(index as usize)
        });
        Some((anchor, column))
    })
}

/// Whether `ch` is one of the private-use codepoints a placement anchors with:
/// either half of the pair.
fn is_anchor(ch: char) -> bool {
    let ch = ch as u32;
    (ANCHOR..ANCHOR + ANCHOR_MAX).contains(&ch)
        || (ANCHOR_COLUMN..ANCHOR_COLUMN + ANCHOR_COLUMN_MAX).contains(&ch)
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
