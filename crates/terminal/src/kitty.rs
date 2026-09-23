//! The kitty graphics protocol, read off the byte stream before the ANSI
//! parser sees it.
//!
//! `vte` throws the protocol away: a graphics command is an APC sequence
//! (`ESC _ G <keys> ; <base64> ESC \`), and `advance_esc` routes `0x5E..=0x5F`
//! into a discard state with no `Perform` hook to implement. So the run never
//! reaches `alacritty_terminal` and no handler can be written to catch it.
//!
//! [`Scanner`] is the way in. It splits the pty stream into the runs the
//! emulator passes through and the commands it keeps, which is also what makes
//! placement possible: fed in order, the grid state at the moment an image
//! arrives is the state the preceding text left behind.
//!
//! What lands here is the transmission half of the protocol — the bytes of an
//! image, assembled and stored under its id. Placements — where an image goes
//! on the grid, and which of them a delete takes off it — are the emulator's,
//! and painting is the view's.

use std::collections::HashMap;

/// The most one APC run may carry before it is abandoned. The protocol chunks
/// payloads at 4096 base64 bytes, so a run far past that is a stream that lost
/// its terminator — and a scanner that kept buffering would be a memory leak
/// driven by whatever is on the other end of the pty.
const MAX_RUN: usize = 1 << 16;

/// The most one assembled image may carry, chunks included: 64 MiB, which is a
/// 4096x4096 image in RGBA with room over.
const MAX_IMAGE: usize = 64 << 20;

/// How many images are kept before the oldest is dropped. Kitty has a
/// storage quota in bytes; this is the same idea at the granularity the store
/// actually evicts at.
const MAX_IMAGES: usize = 64;

// ---------------------------------------------------------------------------
// Scanner
// ---------------------------------------------------------------------------

/// One run of the pty stream.
#[derive(Debug, PartialEq, Eq)]
pub enum Segment<'a> {
    /// Bytes for the ANSI parser, exactly as they arrived.
    Text(&'a [u8]),
    /// A complete graphics command, terminator stripped.
    Graphics(Command),
    /// Mode 2026 turning on (`true`) or off (`false`), reported where it sits
    /// in the stream. The bytes themselves stay in the `Text` run around it,
    /// so the parser still sees the sequence it always saw.
    Sync(bool),
    /// `CSI 16 t`, asking for the cell size in pixels, reported where it sits
    /// in the stream. `vte` drops the sequence without a `Handler` call, so
    /// nothing downstream of the parser can answer it. Its bytes stay in the
    /// `Text` run around it.
    CellSizeQuery,
}

/// Where a [`Scanner`] is in the stream between calls. A pty read ends
/// wherever the kernel filled the buffer, which is as likely to be inside an
/// escape as anywhere else.
#[derive(Debug, Default, PartialEq, Eq)]
enum State {
    /// Passing bytes through.
    #[default]
    Text,
    /// An `ESC` arrived last, and what it introduces is the next byte.
    Escape,
    /// Inside an APC run, accumulating.
    Apc,
    /// Inside an APC run, and the last byte was the `ESC` of a possible `ST`.
    ApcEscape,
    /// Inside a run too long to be a graphics command: swallowed to its
    /// terminator so the payload never reaches the screen as text.
    Overrun,
    /// [`State::Overrun`] having just seen an `ESC`.
    OverrunEscape,
}

/// Splits graphics commands out of the pty stream.
///
/// One per terminal, fed every read in order. Between calls it holds whatever
/// of a command has arrived so far, so a sequence straddling two reads is one
/// command rather than two halves of garbage on the screen.
#[derive(Debug, Default)]
pub struct Scanner {
    state: State,
    run: Vec<u8>,
    /// Bytes of a BSU/ESU run matched so far.
    sync: usize,
    /// Bytes of a `CSI 16 t` matched so far.
    cell_query: usize,
}

impl Scanner {
    pub fn new() -> Self {
        Self::default()
    }

    /// Split `bytes` into the runs the emulator should act on, in order.
    ///
    /// A `Text` segment borrows from `bytes`. An `ESC` held across the call
    /// boundary is re-emitted as its own segment once the byte after it turns
    /// out not to be `_`, so nothing is lost and nothing is duplicated.
    pub fn feed<'a>(&mut self, bytes: &'a [u8]) -> Vec<Segment<'a>> {
        let mut out = Vec::new();
        // Where the current pass-through run began. Text is emitted in slices
        // of the caller's buffer rather than copied.
        let mut text = 0;
        let mut at = 0;
        while at < bytes.len() {
            let byte = bytes[at];
            match self.state {
                State::Text => {
                    let sync = self.sync_step(byte);
                    let cell_query = self.cell_query_step(byte);
                    at += 1;
                    if byte == ESC {
                        push_text(&mut out, &bytes[text..at - 1]);
                        self.state = State::Escape;
                    } else if let Some(hold) = sync {
                        push_text(&mut out, &bytes[text..at]);
                        out.push(Segment::Sync(hold));
                        text = at;
                    } else if cell_query {
                        push_text(&mut out, &bytes[text..at]);
                        out.push(Segment::CellSizeQuery);
                        text = at;
                    }
                }
                State::Escape => {
                    self.state = match byte {
                        APC => {
                            at += 1;
                            // The `ESC` that opened this run counts toward a
                            // BSU/ESU match that the payload cannot finish.
                            self.sync = 0;
                            self.cell_query = 0;
                            State::Apc
                        }
                        // Not ours. The `ESC` was swallowed by the branch
                        // above, so it is handed back on its own and the byte
                        // after it starts the next run.
                        _ => {
                            out.push(Segment::Text(&ESC_BYTES));
                            State::Text
                        }
                    };
                    text = at;
                }
                State::Apc | State::ApcEscape => {
                    if self.state == State::ApcEscape {
                        self.state = State::Apc;
                        if byte == ST {
                            at += 1;
                            text = at;
                            self.finish(&mut out);
                            continue;
                        }
                        // A lone `ESC` inside the payload: keep it and carry
                        // on, since only `ESC \` ends the run.
                        self.run.push(ESC);
                    }
                    match byte {
                        ESC => self.state = State::ApcEscape,
                        // `BEL` terminates a string sequence too, and enough
                        // programs use it for `ESC \` alone to be a gamble.
                        BEL => {
                            at += 1;
                            text = at;
                            self.finish(&mut out);
                            continue;
                        }
                        _ => self.run.push(byte),
                    }
                    at += 1;
                    if self.run.len() > MAX_RUN {
                        self.run.clear();
                        self.state = State::Overrun;
                    }
                    text = at;
                }
                State::Overrun | State::OverrunEscape => {
                    if self.state == State::OverrunEscape {
                        self.state = State::Overrun;
                        if byte == ST {
                            at += 1;
                            text = at;
                            self.state = State::Text;
                            continue;
                        }
                    }
                    match byte {
                        ESC => self.state = State::OverrunEscape,
                        BEL => self.state = State::Text,
                        _ => {}
                    }
                    at += 1;
                    text = at;
                }
            }
        }
        if matches!(self.state, State::Text) {
            push_text(&mut out, &bytes[text..]);
        }
        out
    }

    /// Feed one pass-through byte to the BSU/ESU matcher, answering when one
    /// of the two just completed.
    ///
    /// They are fixed eight-byte strings and matched as such. `vte` finds them
    /// in its own buffer the same way, so a stream it reads as a synchronized
    /// update is a stream this reports.
    fn sync_step(&mut self, byte: u8) -> Option<bool> {
        if self.sync < SYNC_PREFIX.len() {
            self.sync = if byte == SYNC_PREFIX[self.sync] {
                self.sync + 1
            } else {
                usize::from(byte == SYNC_PREFIX[0])
            };
            return None;
        }
        self.sync = usize::from(byte == SYNC_PREFIX[0]);
        match byte {
            b'h' => Some(true),
            b'l' => Some(false),
            _ => None,
        }
    }

    /// Feed one pass-through byte to the `CSI 16 t` matcher, answering whether
    /// it just completed. A fixed string, matched the way [`Self::sync_step`]
    /// matches BSU/ESU.
    fn cell_query_step(&mut self, byte: u8) -> bool {
        self.cell_query = if byte == CELL_SIZE_QUERY[self.cell_query] {
            self.cell_query + 1
        } else {
            usize::from(byte == CELL_SIZE_QUERY[0])
        };
        if self.cell_query == CELL_SIZE_QUERY.len() {
            self.cell_query = 0;
            return true;
        }
        false
    }

    /// End the run being accumulated, keeping it only if it parses.
    fn finish(&mut self, out: &mut Vec<Segment<'_>>) {
        self.state = State::Text;
        let run = std::mem::take(&mut self.run);
        // Every other APC sequence belongs to somebody else — a shell writing
        // its own is not ours to answer, and it was being discarded before
        // this existed.
        if let Some(command) = Command::parse(&run) {
            out.push(Segment::Graphics(command));
        }
    }
}

/// Everything but the final `h`/`l` of `CSI ? 2026 h` and `CSI ? 2026 l`.
const SYNC_PREFIX: &[u8] = b"\x1b[?2026";

/// XTWINOPS 16: report the cell size in pixels.
const CELL_SIZE_QUERY: &[u8] = b"\x1b[16t";

const ESC: u8 = 0x1b;
const ESC_BYTES: [u8; 1] = [ESC];
const APC: u8 = b'_';
const ST: u8 = b'\\';
const BEL: u8 = 0x07;

fn push_text<'a>(out: &mut Vec<Segment<'a>>, bytes: &'a [u8]) {
    if !bytes.is_empty() {
        out.push(Segment::Text(bytes));
    }
}

// ---------------------------------------------------------------------------
// Commands
// ---------------------------------------------------------------------------

/// What a command asks for — kitty's `a` key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Action {
    /// `a=t`: hold the image under its id.
    #[default]
    Transmit,
    /// `a=T`: hold it and put it on the screen at the cursor.
    Display,
    /// `a=q`: answer whether this would have worked, storing nothing.
    Query,
    /// `a=p`: put an image already held on the screen at the cursor.
    Place,
    /// `a=d`: take placements off the screen, and with an upper-case `d`
    /// their images' data too.
    Delete,
    /// `a=f`, `a=c`, `a=a`: parsed so the run is still consumed rather than
    /// printed, and answered as unsupported.
    Other(char),
}

/// How the payload is encoded — kitty's `f` key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    /// `f=24`: raw RGB, `s` by `v` pixels.
    Rgb,
    /// `f=32`: raw RGBA, `s` by `v` pixels.
    Rgba,
    /// `f=100`: a PNG, which carries its own dimensions.
    Png,
}

/// Whether a display leaves the cursor where it found it — kitty's `C` key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CursorMovement {
    /// `C=0`: the cursor ends past the image, scrolling the screen when the
    /// image runs off the bottom.
    #[default]
    After,
    /// `C=1`: neither the cursor nor the screen moves.
    None,
}

/// A parsed graphics command: its keys, and the payload already un-base64'd.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Command {
    pub action: Action,
    pub format: Format,
    /// `i`: the id the client filed the image under. Zero means unset, which
    /// is how kitty spells "the terminal picks one".
    pub id: u32,
    /// `I`: an image number, which names the newest image transmitted under
    /// it. Zero is unset. A command carrying both `i` and `I` is refused.
    pub number: u32,
    /// `p`: the placement id. Zero is unset.
    pub placement: u32,
    /// `m=1`: more chunks follow.
    pub more: bool,
    /// `s`, `v`: the pixel dimensions a raw payload cannot state for itself.
    pub width: u32,
    pub height: u32,
    /// `c`, `r`: the cell extent the image is to be drawn across. Zero is
    /// unset, which leaves the extent to the image's own pixels.
    pub columns: u32,
    pub rows: u32,
    /// `x`, `y`: the cell a delete by position names, 1-based, or the id
    /// range of `d=r`.
    pub x: u32,
    pub y: u32,
    /// `z`: the placement's z-index.
    pub z: i32,
    /// `C`: whether the cursor moves past the image.
    pub cursor_movement: CursorMovement,
    /// `q`: 1 suppresses success replies, 2 suppresses failures too.
    pub quiet: u8,
    /// `d`: what a delete is aimed at; an uppercase letter also frees the
    /// data of the images left with no placement.
    pub delete: char,
    /// `t`: where the bytes are. Only `d` — inline — is read; a file or
    /// shared-memory transfer is a path this does not open.
    pub medium: char,
    /// `o`: payload compression. Only `None` is carried out.
    pub compressed: bool,
    pub payload: Vec<u8>,
}

impl Default for Command {
    fn default() -> Self {
        Self {
            action: Action::Transmit,
            format: Format::Rgba,
            id: 0,
            number: 0,
            placement: 0,
            more: false,
            width: 0,
            height: 0,
            columns: 0,
            rows: 0,
            x: 0,
            y: 0,
            z: 0,
            cursor_movement: CursorMovement::After,
            quiet: 0,
            delete: 'a',
            medium: 'd',
            compressed: false,
            payload: Vec::new(),
        }
    }
}

impl Command {
    /// Parse one APC run's body. `None` for anything that is not a graphics
    /// command, which is every other APC sequence on the wire.
    fn parse(run: &[u8]) -> Option<Self> {
        let (&b'G', rest) = run.split_first()? else {
            return None;
        };
        let (keys, payload) = match rest.iter().position(|&byte| byte == b';') {
            Some(at) => (&rest[..at], &rest[at + 1..]),
            None => (rest, &[][..]),
        };
        let mut command = Command::default();
        for pair in keys.split(|&byte| byte == b',') {
            let Some(at) = pair.iter().position(|&byte| byte == b'=') else {
                continue;
            };
            let (key, value) = (&pair[..at], &pair[at + 1..]);
            let [key] = key else { continue };
            let number = || {
                std::str::from_utf8(value)
                    .ok()
                    .and_then(|value| value.parse::<u32>().ok())
            };
            let letter = || value.first().map(|&byte| byte as char);
            match key {
                b'a' => {
                    command.action = match letter() {
                        Some('t') => Action::Transmit,
                        Some('T') => Action::Display,
                        Some('q') => Action::Query,
                        Some('p') => Action::Place,
                        Some('d') => Action::Delete,
                        Some(other) => Action::Other(other),
                        None => continue,
                    }
                }
                b'f' => {
                    command.format = match number() {
                        Some(24) => Format::Rgb,
                        Some(32) => Format::Rgba,
                        Some(100) => Format::Png,
                        _ => continue,
                    }
                }
                b'i' => command.id = number().unwrap_or(0),
                b'I' => command.number = number().unwrap_or(0),
                b'p' => command.placement = number().unwrap_or(0),
                b'x' => command.x = number().unwrap_or(0),
                b'y' => command.y = number().unwrap_or(0),
                b'z' => {
                    command.z = std::str::from_utf8(value)
                        .ok()
                        .and_then(|value| value.parse::<i32>().ok())
                        .unwrap_or(0)
                }
                b'm' => command.more = number() == Some(1),
                b's' => command.width = number().unwrap_or(0),
                b'v' => command.height = number().unwrap_or(0),
                b'c' => command.columns = number().unwrap_or(0),
                b'r' => command.rows = number().unwrap_or(0),
                b'C' => {
                    command.cursor_movement = match number() {
                        Some(1) => CursorMovement::None,
                        _ => CursorMovement::After,
                    }
                }
                b'q' => command.quiet = number().unwrap_or(0).min(u8::MAX as u32) as u8,
                b'd' => command.delete = letter().unwrap_or('a'),
                b't' => command.medium = letter().unwrap_or('d'),
                b'o' => command.compressed = letter().is_some(),
                _ => {}
            }
        }
        command.payload = decode(payload);
        Some(command)
    }
}

/// Base64, the subset a graphics payload is: standard alphabet, padding
/// optional, anything else skipped. Written out rather than taken as a
/// dependency — the decoder is the size of the code that would configure one.
fn decode(bytes: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(bytes.len() / 4 * 3);
    let mut bits: u32 = 0;
    let mut held = 0;
    for &byte in bytes {
        let six = match byte {
            b'A'..=b'Z' => byte - b'A',
            b'a'..=b'z' => byte - b'a' + 26,
            b'0'..=b'9' => byte - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            _ => continue,
        };
        bits = (bits << 6) | six as u32;
        held += 6;
        if held >= 8 {
            held -= 8;
            out.push((bits >> held) as u8);
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Store
// ---------------------------------------------------------------------------

/// An image the client sent, still in the bytes it sent.
///
/// Decoding is the view's: it is the half that needs a gpui image and a
/// display, and holding a decoded frame here would put one in the emulator's
/// test harness as well.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Image {
    pub format: Format,
    /// The dimensions the client stated. Zero for a [`Format::Png`], which
    /// carries its own.
    pub width: u32,
    pub height: u32,
    pub bytes: Vec<u8>,
}

/// An image put on the screen, and the keys that say how.
///
/// The extent and the cursor rule belong to the placement rather than to the
/// stored image: the same image displayed twice can cover a different number
/// of cells each time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Display {
    pub image: u32,
    /// `p`, zero when unset.
    pub placement: u32,
    /// `c`, `r`, zero when unset.
    pub columns: u32,
    pub rows: u32,
    pub z: i32,
    pub cursor_movement: CursorMovement,
}

/// A delete, which names placements, and placements are the emulator's.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Delete {
    pub target: Target,
    /// An upper-case `d`: the images whose placements this removes lose their
    /// data too, once no placement of theirs is left.
    pub free: bool,
}

/// Which placements a [`Delete`] removes. Cells are 0-based screen
/// coordinates, already converted from the protocol's 1-based ones.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    /// `d=a`: every placement on the screen.
    All,
    /// `d=i`, `d=n`: an image's placements, or the one with this placement
    /// id when it is not zero.
    Image { id: u32, placement: u32 },
    /// `d=c`: the placements covering the cursor's cell.
    Cursor,
    /// `d=p`, and `d=q` when `z` is set: the placements covering a cell.
    Cell { col: u32, row: u32, z: Option<i32> },
    /// `d=x`: the placements crossing a column.
    Column(u32),
    /// `d=y`: the placements crossing a row.
    Row(u32),
    /// `d=z`: the placements with this z-index.
    Z(i32),
    /// `d=r`: the placements of every image id in the range, inclusive.
    Range(u32, u32),
}

/// What the emulator has to carry out after the store has done its part.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Effect {
    Display(Display),
    Delete(Delete),
}

/// What a command asks the terminal to say back, if anything.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reply {
    pub id: u32,
    /// The `I` the command carried, echoed so a client that transmitted by
    /// number can match the answer. Zero when it carried none.
    pub number: u32,
    /// The `p` the command carried. Zero when it carried none.
    pub placement: u32,
    /// `None` is `OK`.
    pub error: Option<&'static str>,
}

impl Reply {
    /// The bytes of the reply, as an APC of its own.
    pub fn bytes(&self) -> Vec<u8> {
        let mut keys = format!("i={}", self.id);
        if self.number != 0 {
            keys.push_str(&format!(",I={}", self.number));
        }
        if self.placement != 0 {
            keys.push_str(&format!(",p={}", self.placement));
        }
        let body = self.error.unwrap_or("OK");
        format!("\x1b_G{keys};{body}\x1b\\").into_bytes()
    }
}

/// The images a terminal is holding, and the chunks of the ones still arriving.
#[derive(Debug, Default)]
pub struct Store {
    images: HashMap<u32, Image>,
    /// Insertion order, for the eviction [`MAX_IMAGES`] forces.
    order: Vec<u32>,
    /// A transmission still being chunked: its id, its keys, and what has
    /// arrived. One at a time — kitty allows no second transmission to begin
    /// before the first ends.
    pending: Option<(u32, Command)>,
    next_id: u32,
    /// Image numbers to the id of the newest image transmitted under each.
    numbers: HashMap<u32, u32>,
}

impl Store {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get(&self, id: u32) -> Option<&Image> {
        self.images.get(&id)
    }

    pub fn len(&self) -> usize {
        self.images.len()
    }

    pub fn is_empty(&self) -> bool {
        self.images.is_empty()
    }

    /// Free one image's data.
    pub(crate) fn remove(&mut self, id: u32) {
        self.images.remove(&id);
        self.order.retain(|held| *held != id);
        self.numbers.retain(|_, held| *held != id);
    }

    /// Free every image's data.
    pub(crate) fn clear(&mut self) {
        self.images.clear();
        self.order.clear();
        self.numbers.clear();
    }

    /// Carry out one command. The reply is what the client asked to hear:
    /// `None` when it asked for silence, and always `None` for a chunk that is
    /// not the last — an answer per chunk would be an answer per 4096 bytes.
    ///
    /// Answers with the [`Effect`] the emulator carries out: the placement an
    /// `a=T` or `a=p` resolved to, or the placements a delete names.
    pub fn apply(&mut self, command: Command) -> (Option<Effect>, Option<Reply>) {
        if command.id != 0 && command.number != 0 {
            return (
                None,
                self.reply(&command, command.id, Some("EINVAL:i and I")),
            );
        }
        let id = match (command.id, command.number) {
            (0, 0) => self.pending.as_ref().map_or(0, |(id, _)| *id),
            // A transmission by number files a new image; anything else names
            // the newest one filed under it.
            (0, _) if matches!(command.action, Action::Transmit | Action::Display) => 0,
            (0, number) => self.numbers.get(&number).copied().unwrap_or(0),
            (id, _) => id,
        };
        match command.action {
            Action::Query => (None, self.reply(&command, id, None)),
            Action::Place => {
                if !self.images.contains_key(&id) {
                    return (None, self.reply(&command, id, Some("ENOENT:image")));
                }
                let display = Display {
                    image: id,
                    placement: command.placement,
                    columns: command.columns,
                    rows: command.rows,
                    z: command.z,
                    cursor_movement: command.cursor_movement,
                };
                (
                    Some(Effect::Display(display)),
                    self.reply(&command, id, None),
                )
            }
            Action::Delete => {
                let target = match command.delete.to_ascii_lowercase() {
                    'a' => Target::All,
                    'i' | 'n' => Target::Image {
                        id,
                        placement: command.placement,
                    },
                    'c' => Target::Cursor,
                    'p' | 'q' => Target::Cell {
                        col: command.x.saturating_sub(1),
                        row: command.y.saturating_sub(1),
                        z: (command.delete.eq_ignore_ascii_case(&'q')).then_some(command.z),
                    },
                    'x' => Target::Column(command.x.saturating_sub(1)),
                    'y' => Target::Row(command.y.saturating_sub(1)),
                    'z' => Target::Z(command.z),
                    'r' => Target::Range(command.x, command.y),
                    'f' => {
                        return (None, self.reply(&command, id, Some("ENOTSUPPORTED:delete")));
                    }
                    _ => return (None, self.reply(&command, id, Some("EINVAL:delete"))),
                };
                let delete = Delete {
                    target,
                    free: command.delete.is_ascii_uppercase(),
                };
                (Some(Effect::Delete(delete)), self.reply(&command, id, None))
            }
            Action::Other(_) => (None, self.reply(&command, id, Some("ENOTSUPPORTED:action"))),
            Action::Transmit | Action::Display => self.transmit(command, id),
        }
    }

    fn transmit(&mut self, command: Command, id: u32) -> (Option<Effect>, Option<Reply>) {
        if command.compressed {
            self.pending = None;
            return (
                None,
                self.reply(&command, id, Some("ENOTSUPPORTED:compression")),
            );
        }
        if command.medium != 'd' {
            self.pending = None;
            return (None, self.reply(&command, id, Some("ENOTSUPPORTED:medium")));
        }
        // An id of its own for a client that sent none, so everything in the
        // store can be named — by a delete, or by the placement this becomes.
        let id = match id {
            0 => {
                self.next_id = self.next_id.wrapping_add(1).max(1);
                self.next_id
            }
            id => id,
        };

        let mut held = match self.pending.take() {
            Some((pending, held)) if pending == id => held,
            // A fresh transmission abandons an unfinished one: two at once is
            // outside the protocol, and holding the old chunks would splice
            // one image into the other.
            _ => Command {
                payload: Vec::new(),
                ..command.clone()
            },
        };
        // Kitty states a transmission's control keys on its first chunk, and
        // the chunk that ends it carries `m=0` and little else. `held` is
        // where those keys live, so the silence the client asked for is read
        // from there rather than from the chunk in hand.
        let mut command = command;
        command.quiet = command.quiet.max(held.quiet);
        if held.payload.len() + command.payload.len() > MAX_IMAGE {
            return (None, self.reply(&command, id, Some("EFBIG:payload")));
        }
        held.payload.extend_from_slice(&command.payload);
        // The action lives on the first chunk in kitty's own client, and on
        // the last in others. Either is a display, and the display keys ride
        // whichever chunk carries it.
        if command.action == Action::Display {
            held.action = Action::Display;
        }
        held.columns = held.columns.max(command.columns);
        held.rows = held.rows.max(command.rows);
        held.placement = held.placement.max(command.placement);
        held.number = held.number.max(command.number);
        if command.z != 0 {
            held.z = command.z;
        }
        if command.cursor_movement == CursorMovement::None {
            held.cursor_movement = CursorMovement::None;
        }
        if command.more {
            self.pending = Some((id, held));
            return (None, None);
        }
        // The closing chunk carries none of the keys a reply echoes.
        command.number = held.number;
        command.placement = held.placement;

        if held.payload.is_empty() {
            return (None, self.reply(&command, id, Some("EINVAL:empty")));
        }
        let image = Image {
            format: held.format,
            width: held.width,
            height: held.height,
            bytes: held.payload,
        };
        if !matches!(image.format, Format::Png) && !raw_fits(&image) {
            return (None, self.reply(&command, id, Some("EINVAL:dimensions")));
        }
        self.insert(id, image);
        if held.number != 0 {
            self.numbers.insert(held.number, id);
        }
        let landed = (held.action == Action::Display).then_some(Effect::Display(Display {
            image: id,
            placement: held.placement,
            columns: held.columns,
            rows: held.rows,
            z: held.z,
            cursor_movement: held.cursor_movement,
        }));
        (landed, self.reply(&command, id, None))
    }

    fn insert(&mut self, id: u32, image: Image) {
        if self.images.insert(id, image).is_none() {
            self.order.push(id);
        }
        while self.order.len() > MAX_IMAGES {
            let oldest = self.order[0];
            self.remove(oldest);
        }
    }

    /// `q=1` silences a success, `q=2` silences a failure as well. A
    /// transmission with no id to name is not answered either: kitty's reply
    /// grammar has nowhere to put the answer.
    fn reply(&self, command: &Command, id: u32, error: Option<&'static str>) -> Option<Reply> {
        let reply = Reply {
            id,
            number: command.number,
            placement: command.placement,
            error,
        };
        match error {
            Some(_) if command.quiet < 2 => Some(reply),
            Some(_) => None,
            None if command.quiet < 1 && id != 0 => Some(reply),
            None => None,
        }
    }
}

impl Image {
    /// The image's pixel dimensions: the ones the client stated, or the ones a
    /// PNG states for itself. `None` for a PNG too short or too malformed to
    /// say, which is a payload nothing will decode either.
    pub fn size(&self) -> Option<(u32, u32)> {
        match self.format {
            Format::Png => png_size(&self.bytes),
            _ => (self.width > 0 && self.height > 0).then_some((self.width, self.height)),
        }
    }
}

/// A PNG's dimensions, off the `IHDR` that opens every one of them: 8 bytes of
/// signature, a 4-byte length, the `IHDR` tag, then width and height.
///
/// Read here rather than decoded, because the size is what the *grid* needs —
/// how many cells the image will cover, and so where the cursor lands after
/// it. Decoding is the view's, a frame later and only if the image is on
/// screen.
fn png_size(bytes: &[u8]) -> Option<(u32, u32)> {
    const SIGNATURE: &[u8] = b"\x89PNG\r\n\x1a\n";
    if !bytes.starts_with(SIGNATURE) || bytes.len() < 24 || &bytes[12..16] != b"IHDR" {
        return None;
    }
    let number =
        |at: usize| -> Option<u32> { Some(u32::from_be_bytes(bytes[at..at + 4].try_into().ok()?)) };
    Some((number(16)?, number(20)?))
}

/// Whether a raw payload holds the pixels its dimensions claim. A client that
/// states one size and sends another would otherwise be read past its buffer
/// at paint time.
fn raw_fits(image: &Image) -> bool {
    let channels = match image.format {
        Format::Rgb => 3usize,
        Format::Rgba => 4,
        Format::Png => return true,
    };
    let claimed = (image.width as usize)
        .checked_mul(image.height as usize)
        .and_then(|pixels| pixels.checked_mul(channels));
    matches!(claimed, Some(claimed) if claimed > 0 && image.bytes.len() >= claimed)
}
