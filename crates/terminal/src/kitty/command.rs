//! Commands: the parsed keys of one graphics escape, and the payload decoder.

/// What a command asks for — kitty's `a` key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[non_exhaustive]
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
    /// `a=f`: add a frame to an image, or draw over one it has.
    Frame,
    /// `a=a`: set an animation's state, current frame, loops or gaps.
    Animate,
    /// `a=c`: copy a rectangle of one frame onto another.
    Compose,
    /// Any other letter: parsed so the run is still consumed rather than
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
#[non_exhaustive]
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
    /// `x`, `y`: on a display, the source rectangle's top-left in the
    /// image's pixels. On a delete, the 1-based cell a delete by position
    /// names, or the id range of `d=r`.
    pub x: u32,
    pub y: u32,
    /// `w`, `h`: the source rectangle's size in pixels. Zero runs to the
    /// image's edge.
    pub source_width: u32,
    pub source_height: u32,
    /// `X`, `Y`: where in its top-left cell the image starts, in pixels.
    pub offset_x: u32,
    pub offset_y: u32,
    /// `z`: the placement's z-index.
    pub z: i32,
    /// `C`: whether the cursor moves past the image.
    pub cursor_movement: CursorMovement,
    /// `P`, `Q`: the image and placement a relative placement hangs off.
    /// A `P` of zero is no parent.
    pub parent_image: u32,
    pub parent_placement: u32,
    /// `H`, `V`: a relative placement's offset from its parent, in cells.
    pub parent_offset_x: i32,
    pub parent_offset_y: i32,
    /// `U=1`: the placement is virtual, shown only where the text holds
    /// [`PLACEHOLDER`] cells naming it.
    pub unicode: bool,
    /// `q`: 1 suppresses success replies, 2 suppresses failures too.
    pub quiet: u8,
    /// `d`: what a delete is aimed at; an uppercase letter also frees the
    /// data of the images left with no placement.
    pub delete: char,
    /// `t`: where the bytes are. `d` carries them inline; `f`, `t` and `s`
    /// name a file, a temporary file or a shared memory object, read only
    /// once [`Store::set_local_media`] allows it.
    pub medium: char,
    /// `O`, `S`: where in a named medium the bytes start, and how many to
    /// read. A size of zero reads to the end.
    pub read_offset: u32,
    pub read_size: u32,
    /// `o`: payload compression, `None` when there is none. Only `z`, zlib,
    /// is carried out.
    pub compression: Option<char>,
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
            source_width: 0,
            source_height: 0,
            offset_x: 0,
            offset_y: 0,
            z: 0,
            cursor_movement: CursorMovement::After,
            parent_image: 0,
            parent_placement: 0,
            parent_offset_x: 0,
            parent_offset_y: 0,
            unicode: false,
            quiet: 0,
            delete: 'a',
            medium: 'd',
            read_offset: 0,
            read_size: 0,
            compression: None,
            payload: Vec::new(),
        }
    }
}

impl Command {
    /// Parse one APC run's body. `None` for anything that is not a graphics
    /// command, which is every other APC sequence on the wire.
    pub(crate) fn parse(run: &[u8]) -> Option<Self> {
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
            let signed = || {
                std::str::from_utf8(value)
                    .ok()
                    .and_then(|value| value.parse::<i32>().ok())
            };
            match key {
                b'a' => {
                    command.action = match letter() {
                        Some('t') => Action::Transmit,
                        Some('T') => Action::Display,
                        Some('q') => Action::Query,
                        Some('p') => Action::Place,
                        Some('d') => Action::Delete,
                        Some('f') => Action::Frame,
                        Some('a') => Action::Animate,
                        Some('c') => Action::Compose,
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
                b'w' => command.source_width = number().unwrap_or(0),
                b'h' => command.source_height = number().unwrap_or(0),
                b'X' => command.offset_x = number().unwrap_or(0),
                b'Y' => command.offset_y = number().unwrap_or(0),
                b'z' => command.z = signed().unwrap_or(0),
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
                b'U' => command.unicode = number() == Some(1),
                b'P' => command.parent_image = number().unwrap_or(0),
                b'Q' => command.parent_placement = number().unwrap_or(0),
                b'H' => command.parent_offset_x = signed().unwrap_or(0),
                b'V' => command.parent_offset_y = signed().unwrap_or(0),
                b'q' => command.quiet = number().unwrap_or(0).min(u8::MAX as u32) as u8,
                b'd' => command.delete = letter().unwrap_or('a'),
                b't' => command.medium = letter().unwrap_or('d'),
                b'O' => command.read_offset = number().unwrap_or(0),
                b'S' => command.read_size = number().unwrap_or(0),
                b'o' => command.compression = letter(),
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
pub(crate) fn decode(bytes: &[u8]) -> Vec<u8> {
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
            // Padding ends a group: a new one starts on a byte boundary,
            // which is where a second base64 string run on after it begins.
            b'=' => {
                held = 0;
                continue;
            }
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
