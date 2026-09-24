//! The pty stream, split before the ANSI parser sees it: the text the parser
//! takes, and the sequences it would throw away or never answer.
//!
//! `vte` throws kitty graphics away: a graphics command is an APC sequence
//! (`ESC _ G <keys> ; <base64> ESC \`), and `advance_esc` routes `0x5E..=0x5F`
//! into a discard state with no `Perform` hook to implement. A sixel image's
//! data (DCS) and an iTerm2 file (OSC 1337) reach handlers `Term` leaves
//! empty, and `CSI 16 t` is dropped without a `Handler` call.
//!
//! [`Scanner`] reports each where it sits in the stream: fed in order, the
//! grid state at the moment an image arrives is the state the preceding text
//! left behind.

use std::path::PathBuf;

use crate::kitty::Command;

/// The most one APC run may carry before it is abandoned. The protocol chunks
/// payloads at 4096 base64 bytes, so a run far past that is a stream that lost
/// its terminator — and a scanner that kept buffering would be a memory leak
/// driven by whatever is on the other end of the pty.
const MAX_RUN: usize = 1 << 16;

/// One run of the pty stream.
#[derive(Debug, PartialEq, Eq)]
#[non_exhaustive]
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
    /// A sixel image's data: what sits between `DCS P1;P2;P3 q` and `ST`.
    /// The parser is handed the DCS with its data taken out, so it leaves the
    /// sequence in the state it would have.
    Sixel(Vec<u8>),
    /// An iTerm2 inline image command. The parser is handed the OSC with any
    /// payload taken out.
    Iterm(Iterm),
    /// The shell's working directory, from `OSC 7 ; file://host/path` or
    /// `OSC 9 ; 9 ; path`. The host is dropped: a shell over ssh reports the
    /// remote machine's path. The OSC's bytes stay in the `Text` run around
    /// it.
    Directory(PathBuf),
}

/// An `OSC 1337` command that carries a file. Arguments and payloads are as
/// they arrived: `key=value;…` text and base64.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Iterm {
    /// `File=<args>:<payload>`.
    File { args: Vec<u8>, payload: Vec<u8> },
    /// `MultipartFile=<args>`: a file follows in parts.
    Begin { args: Vec<u8> },
    /// `FilePart=<payload>`.
    Part(Vec<u8>),
    /// `FileEnd`.
    End,
}

/// Which `OSC 1337` command a header named.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OscKind {
    File,
    Begin,
    Part,
    End,
    /// `OSC 7`.
    FileUrl,
    /// `OSC 9 ; 9`.
    Path,
}

/// The OSC commands the scanner reports, by the header that starts each.
const OSC_HEADERS: [(&[u8], OscKind); 6] = [
    (b"1337;File=", OscKind::File),
    (b"1337;MultipartFile=", OscKind::Begin),
    (b"1337;FilePart=", OscKind::Part),
    (b"1337;FileEnd", OscKind::End),
    (b"7;", OscKind::FileUrl),
    (b"9;9;", OscKind::Path),
];

/// The most an `OSC 1337` payload may carry before it is abandoned.
const MAX_OSC_PAYLOAD: usize = 96 << 20;

/// The longest `OSC 1337` argument list kept.
const MAX_OSC_ARGS: usize = 4096;

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
    /// After `ESC P`, reading a DCS's parameters to learn whether it is a
    /// sixel image. They pass through to the parser either way.
    DcsHeader,
    /// Inside a sixel image's data, accumulating.
    Sixel,
    /// [`State::Sixel`] having just seen an `ESC`.
    SixelEscape,
    /// Inside sixel data too long to keep: swallowed to its terminator.
    SixelOverrun,
    /// [`State::SixelOverrun`] having just seen an `ESC`.
    SixelOverrunEscape,
    /// After `ESC ]`, matching an OSC's start against [`OSC_HEADERS`]. It
    /// passes through to the parser either way.
    OscHeader,
    /// An `OSC 1337` command's arguments, passing through as they are kept.
    OscArgs(OscKind),
    /// [`State::OscArgs`] having just seen an `ESC`.
    OscArgsEscape(OscKind),
    /// An `OSC 1337` payload, kept from the parser.
    OscPayload(OscKind),
    /// [`State::OscPayload`] having just seen an `ESC`.
    OscPayloadEscape(OscKind),
}

/// The most sixel data one image may carry before it is abandoned.
const MAX_SIXEL: usize = 32 << 20;

/// The longest DCS parameter string read while deciding on a sixel image.
const MAX_DCS_HEADER: usize = 64;

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
    /// An `OSC 1337` file's arguments, held while its payload arrives.
    args: Vec<u8>,
    /// The `OSC 1337` payload arriving outran [`MAX_OSC_PAYLOAD`] and is
    /// being dropped.
    discard: bool,
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
                        OSC => {
                            at += 1;
                            self.sync = 0;
                            self.cell_query = 0;
                            self.run.clear();
                            out.push(Segment::Text(&OSC_BYTES));
                            State::OscHeader
                        }
                        DCS => {
                            at += 1;
                            self.sync = 0;
                            self.cell_query = 0;
                            self.run.clear();
                            out.push(Segment::Text(&DCS_BYTES));
                            State::DcsHeader
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
                State::OscHeader => {
                    self.run.push(byte);
                    let head = self.run.as_slice();
                    let kind = OSC_HEADERS
                        .iter()
                        .find(|(header, _)| *header == head)
                        .map(|(_, kind)| *kind);
                    if let Some(kind) = kind {
                        at += 1;
                        self.run.clear();
                        self.state = match kind {
                            OscKind::Part => {
                                push_text(&mut out, &bytes[text..at]);
                                text = at;
                                self.discard = false;
                                State::OscPayload(kind)
                            }
                            _ => State::OscArgs(kind),
                        };
                    } else if OSC_HEADERS
                        .iter()
                        .any(|(header, _)| header.starts_with(head))
                    {
                        at += 1;
                    } else {
                        // Not ours: the byte is looked at again as text.
                        self.run.clear();
                        self.state = State::Text;
                    }
                }
                State::OscArgs(kind) => match byte {
                    b':' if kind == OscKind::File => {
                        at += 1;
                        push_text(&mut out, &bytes[text..at]);
                        text = at;
                        self.args = std::mem::take(&mut self.run);
                        self.discard = false;
                        self.state = State::OscPayload(kind);
                    }
                    BEL => {
                        at += 1;
                        push_text(&mut out, &bytes[text..at]);
                        text = at;
                        self.end_args(kind, &mut out);
                    }
                    ESC => {
                        at += 1;
                        self.state = State::OscArgsEscape(kind);
                    }
                    _ if self.run.len() < MAX_OSC_ARGS => {
                        self.run.push(byte);
                        at += 1;
                    }
                    _ => {
                        self.run.clear();
                        self.state = State::Text;
                    }
                },
                State::OscArgsEscape(kind) => {
                    if byte == ST {
                        at += 1;
                        push_text(&mut out, &bytes[text..at]);
                        text = at;
                        self.end_args(kind, &mut out);
                    } else {
                        self.run.clear();
                        self.state = State::Text;
                    }
                }
                State::OscPayload(kind) => {
                    match byte {
                        BEL => {
                            // The parser leaves its OSC before the image
                            // lands, as it does for sixel.
                            out.push(Segment::Text(&BEL_BYTES));
                            self.end_payload(kind, &mut out);
                        }
                        ESC => self.state = State::OscPayloadEscape(kind),
                        _ if self.discard => {}
                        _ if self.run.len() >= MAX_OSC_PAYLOAD => {
                            self.run.clear();
                            self.discard = true;
                        }
                        _ => self.run.push(byte),
                    }
                    at += 1;
                    text = at;
                }
                State::OscPayloadEscape(kind) => {
                    out.push(Segment::Text(&ST_BYTES));
                    self.end_payload(kind, &mut out);
                    if byte == ST {
                        at += 1;
                    } else {
                        out.push(Segment::Text(&ESC_BYTES));
                    }
                    text = at;
                }
                State::DcsHeader => match byte {
                    b'0'..=b'9' | b';' if self.run.len() < MAX_DCS_HEADER => {
                        self.run.push(byte);
                        at += 1;
                    }
                    b'q' => {
                        at += 1;
                        push_text(&mut out, &bytes[text..at]);
                        text = at;
                        self.run.clear();
                        self.state = State::Sixel;
                    }
                    // Not a sixel image: the rest of it is the parser's.
                    _ => {
                        self.run.clear();
                        self.state = State::Text;
                    }
                },
                State::Sixel | State::SixelEscape => {
                    if self.state == State::SixelEscape {
                        // The parser leaves its DCS before the image lands,
                        // so the rows the image reserves reach the grid.
                        out.push(Segment::Text(&ST_BYTES));
                        out.push(Segment::Sixel(std::mem::take(&mut self.run)));
                        self.state = State::Text;
                        if byte == ST {
                            at += 1;
                        } else {
                            // An escape ends a DCS, and this one starts
                            // whatever comes next.
                            out.push(Segment::Text(&ESC_BYTES));
                        }
                        text = at;
                        continue;
                    }
                    match byte {
                        ESC => self.state = State::SixelEscape,
                        // CAN and SUB abandon the sequence. The parser is
                        // handed the byte, which ends the DCS it holds.
                        CAN | SUB => {
                            self.run.clear();
                            self.state = State::Text;
                            text = at;
                            continue;
                        }
                        _ => self.run.push(byte),
                    }
                    at += 1;
                    text = at;
                    if self.run.len() > MAX_SIXEL {
                        self.run.clear();
                        if self.state == State::Sixel {
                            self.state = State::SixelOverrun;
                        }
                    }
                }
                State::SixelOverrun | State::SixelOverrunEscape => {
                    if self.state == State::SixelOverrunEscape {
                        self.state = State::Text;
                        if byte == ST {
                            at += 1;
                            out.push(Segment::Text(&ST_BYTES));
                        } else {
                            out.push(Segment::Text(&ESC_BYTES));
                        }
                        text = at;
                        continue;
                    }
                    match byte {
                        ESC => self.state = State::SixelOverrunEscape,
                        CAN | SUB => {
                            self.state = State::Text;
                            text = at;
                            continue;
                        }
                        _ => {}
                    }
                    at += 1;
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
        if matches!(
            self.state,
            State::Text
                | State::DcsHeader
                | State::OscHeader
                | State::OscArgs(_)
                | State::OscArgsEscape(_)
        ) {
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

    /// An `OSC 1337` command with no payload is over.
    fn end_args(&mut self, kind: OscKind, out: &mut Vec<Segment<'_>>) {
        let args = std::mem::take(&mut self.run);
        self.state = State::Text;
        match kind {
            OscKind::Begin => out.push(Segment::Iterm(Iterm::Begin { args })),
            OscKind::End => out.push(Segment::Iterm(Iterm::End)),
            OscKind::FileUrl => out.extend(file_url(&args).map(Segment::Directory)),
            OscKind::Path => out.extend(quoted_path(&args).map(Segment::Directory)),
            // A file with no `:` has nothing to show.
            OscKind::File | OscKind::Part => {}
        }
    }

    /// An `OSC 1337` payload is over.
    fn end_payload(&mut self, kind: OscKind, out: &mut Vec<Segment<'_>>) {
        let payload = std::mem::take(&mut self.run);
        let args = std::mem::take(&mut self.args);
        self.state = State::Text;
        if std::mem::take(&mut self.discard) {
            return;
        }
        match kind {
            OscKind::File => out.push(Segment::Iterm(Iterm::File { args, payload })),
            OscKind::Part => out.push(Segment::Iterm(Iterm::Part(payload))),
            OscKind::Begin | OscKind::End | OscKind::FileUrl | OscKind::Path => {}
        }
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
const DCS: u8 = b'P';
const OSC: u8 = b']';
const OSC_BYTES: [u8; 2] = [ESC, OSC];
const BEL_BYTES: [u8; 1] = [BEL];
const DCS_BYTES: [u8; 2] = [ESC, DCS];
const ST_BYTES: [u8; 2] = [ESC, b'\\'];
const CAN: u8 = 0x18;
const SUB: u8 = 0x1a;
const ST: u8 = b'\\';
const BEL: u8 = 0x07;

/// The path of a `file://host/path` URL, percent-decoded. A Windows drive path
/// arrives as `/C:/...` and loses the leading slash.
fn file_url(url: &[u8]) -> Option<PathBuf> {
    let rest = url.strip_prefix(b"file://")?;
    let path = &rest[rest.iter().position(|&byte| byte == b'/')?..];
    let mut decoded = Vec::with_capacity(path.len());
    let mut at = 0;
    while at < path.len() {
        let escaped = (path[at] == b'%')
            .then(|| path.get(at + 1..at + 3))
            .flatten()
            .and_then(|hex| u8::from_str_radix(std::str::from_utf8(hex).ok()?, 16).ok());
        match escaped {
            Some(byte) => {
                decoded.push(byte);
                at += 3;
            }
            None => {
                decoded.push(path[at]);
                at += 1;
            }
        }
    }
    if let [b'/', drive, b':', ..] = decoded[..]
        && drive.is_ascii_alphabetic()
    {
        decoded.remove(0);
    }
    path_from(decoded)
}

/// An `OSC 9 ; 9` path, with the quotes Windows Terminal's snippets put
/// around it taken off.
fn quoted_path(path: &[u8]) -> Option<PathBuf> {
    let path = match path {
        [b'"', inner @ .., b'"'] => inner,
        _ => path,
    };
    path_from(path.to_vec())
}

fn path_from(bytes: Vec<u8>) -> Option<PathBuf> {
    if bytes.is_empty() {
        return None;
    }
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStringExt;
        Some(PathBuf::from(std::ffi::OsString::from_vec(bytes)))
    }
    #[cfg(not(unix))]
    {
        String::from_utf8(bytes).ok().map(PathBuf::from)
    }
}

fn push_text<'a>(out: &mut Vec<Segment<'a>>, bytes: &'a [u8]) {
    if !bytes.is_empty() {
        out.push(Segment::Text(bytes));
    }
}
