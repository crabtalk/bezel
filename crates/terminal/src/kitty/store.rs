//! Store: transmitted images, their frames and animation, and the replies.

use super::*;

/// An image the client sent, still in the bytes it sent until it becomes an
/// animation.
///
/// The first frame command on an image decodes it here, into
/// [`Format::Rgba`]. A still image is decoded by the view.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Image {
    pub format: Format,
    /// The dimensions the client stated. Zero for a [`Format::Png`], which
    /// carries its own.
    pub width: u32,
    pub height: u32,
    /// The first frame.
    pub bytes: Vec<u8>,
    /// Every frame after the first, RGBA at the image's size. Empty for a
    /// still image.
    pub frames: Vec<Vec<u8>>,
    /// Each frame's gap in milliseconds, the first frame's first. Zero is a
    /// gapless frame, skipped over. Empty until a frame command sets one.
    pub gaps: Vec<u32>,
    pub animation: Animation,
    /// Bumped whenever a frame's pixels change.
    pub revision: u64,
    /// The [`Self::revision`] each frame's pixels last changed at, the first
    /// frame's first. Distinct across an image's frames. Shorter than the
    /// frames where a frame has never changed; a missing one is zero.
    pub frame_revisions: Vec<u64>,
}

/// How an animation plays — kitty's `a=a`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Animation {
    pub state: AnimationState,
    /// The frame shown while stopped and the one a run starts from, 0-based.
    pub current: usize,
    /// Loops to play before stopping on the last frame. Zero plays forever.
    pub loops: u32,
    /// Bumped whenever the state, the current frame or the loop count is
    /// set, each of which starts playback over from [`Self::current`].
    pub revision: u64,
}

/// kitty's `s` on `a=a`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AnimationState {
    /// `s=1`, and where every image starts.
    #[default]
    Stopped,
    /// `s=2`: play, and wait at the last frame for more rather than loop.
    Loading,
    /// `s=3`: play, looping.
    Running,
}

/// The gap a new frame gets when its command names none.
pub(super) const DEFAULT_GAP: u32 = 40;

/// The most the frames after the first may hold, across every image.
pub(super) const MAX_FRAME_BYTES: usize = 320 << 20;

/// An image put on the screen, and the keys that say how.
///
/// The extent and the cursor rule belong to the placement rather than to the
/// stored image: the same image displayed twice can cover a different number
/// of cells each time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct Display {
    pub image: u32,
    /// `p`, zero when unset.
    pub placement: u32,
    /// `c`, `r`, zero when unset.
    pub columns: u32,
    pub rows: u32,
    /// `x`, `y`, `w`, `h`, as the command carried them.
    pub source_x: u32,
    pub source_y: u32,
    pub source_width: u32,
    pub source_height: u32,
    /// `X`, `Y`, as the command carried them.
    pub offset_x: u32,
    pub offset_y: u32,
    pub z: i32,
    pub cursor_movement: CursorMovement,
    /// `U=1`.
    pub unicode: bool,
    /// `P`, `Q`, `H`, `V`, as the command carried them.
    pub parent_image: u32,
    pub parent_placement: u32,
    pub parent_offset_x: i32,
    pub parent_offset_y: i32,
    /// `q`, for the errors only the emulator can find.
    pub quiet: u8,
    /// With both `c` and `r`, fill the box rather than fit inside it.
    pub stretch: bool,
}

impl Display {
    /// `image` at the cursor, with every key at its default.
    pub(crate) fn at_cursor(image: u32) -> Self {
        Self::of(image, &Command::default())
    }

    /// The display keys a command carried, for the image `image`.
    fn of(image: u32, command: &Command) -> Self {
        Self {
            image,
            placement: command.placement,
            columns: command.columns,
            rows: command.rows,
            source_x: command.x,
            source_y: command.y,
            source_width: command.source_width,
            source_height: command.source_height,
            offset_x: command.offset_x,
            offset_y: command.offset_y,
            z: command.z,
            cursor_movement: command.cursor_movement,
            unicode: command.unicode,
            parent_image: command.parent_image,
            parent_placement: command.parent_placement,
            parent_offset_x: command.parent_offset_x,
            parent_offset_y: command.parent_offset_y,
            quiet: command.quiet,
            stretch: false,
        }
    }
}

/// A delete, which names placements, and placements are the emulator's.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct Delete {
    pub target: Target,
    /// An upper-case `d`: the images whose placements this removes lose their
    /// data too, once no placement of theirs is left.
    pub free: bool,
}

/// Which placements a [`Delete`] removes. Cells are 0-based screen
/// coordinates, already converted from the protocol's 1-based ones.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
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
#[non_exhaustive]
pub enum Effect {
    Display(Display),
    Delete(Delete),
}

/// What a command asks the terminal to say back, if anything.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
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
    pub(super) images: HashMap<u32, Image>,
    /// Insertion order, for the eviction [`MAX_IMAGES`] forces.
    order: Vec<u32>,
    /// A transmission still being chunked: its id, its keys, and what has
    /// arrived. One at a time — kitty allows no second transmission to begin
    /// before the first ends.
    pending: Option<(u32, Command)>,
    next_id: u32,
    /// Image numbers to the id of the newest image transmitted under each.
    numbers: HashMap<u32, u32>,
    /// Whether `t=f`, `t=t` and `t=s` are read.
    local_media: bool,
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

    /// Read the files and shared memory objects a transmission names. Off
    /// until a host turns it on. The paths are resolved on this machine,
    /// whichever machine the program sending them runs on.
    pub fn set_local_media(&mut self, allow: bool) {
        self.local_media = allow;
    }

    /// Hold an image that arrived by some other protocol, under an id of its
    /// own.
    pub(crate) fn hold(&mut self, image: Image) -> u32 {
        self.next_id = self.next_id.wrapping_add(1).max(1);
        let id = self.next_id;
        self.insert(id, image);
        id
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
            Action::Query => {
                let outcome = self.query(&command);
                (None, self.reply_to(&command, id, outcome))
            }
            Action::Place => {
                if !self.images.contains_key(&id) {
                    return (None, self.reply(&command, id, Some("ENOENT:image")));
                }
                let display = Display::of(id, &command);
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
                        let outcome = self.delete_frame(&command, id);
                        return (None, self.reply_to(&command, id, outcome));
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
            Action::Animate => {
                let outcome = self.animate(&command, id);
                (None, self.reply_to(&command, id, outcome))
            }
            Action::Compose => {
                let outcome = self.compose(&command, id);
                (None, self.reply_to(&command, id, outcome))
            }
            Action::Transmit | Action::Display | Action::Frame => self.transmit(command, id),
        }
    }

    /// [`Self::reply`] for an outcome.
    fn reply_to(
        &self,
        command: &Command,
        id: u32,
        outcome: Result<(), &'static str>,
    ) -> Option<Reply> {
        self.reply(command, id, outcome.err())
    }

    /// Why a transmission is refused before any of it is read: a compression
    /// or a medium not carried out.
    fn refused(&self, command: &Command) -> Option<&'static str> {
        if command
            .compression
            .is_some_and(|compression| compression != 'z')
        {
            return Some("ENOTSUPPORTED:compression");
        }
        match command.medium {
            'd' => None,
            'f' | 't' | 's' if self.local_media => None,
            'f' | 't' | 's' => Some("ENOTSUPPORTED:medium"),
            _ => Some("EINVAL:medium"),
        }
    }

    /// `a=q`: whether the same keys and payload would transmit, storing
    /// nothing. A named medium is read — and a temporary file deleted — as it
    /// would be. A query carrying no payload is a bare probe and answered yes.
    fn query(&self, command: &Command) -> Result<(), &'static str> {
        if let Some(refused) = self.refused(command) {
            return Err(refused);
        }
        if command.medium == 'd' && command.payload.is_empty() {
            return Ok(());
        }
        let bytes = load(command, command.payload.clone())?;
        let size = match command.format {
            Format::Png => png_size(&bytes),
            _ => Some((command.width, command.height)),
        };
        let image = Image::still(command.format, command.width, command.height, bytes);
        match size {
            Some(_) if image.format == Format::Png || raw_fits(&image) => Ok(()),
            _ => Err("EINVAL:dimensions"),
        }
    }

    fn transmit(&mut self, command: Command, id: u32) -> (Option<Effect>, Option<Reply>) {
        if command.action == Action::Frame && !self.images.contains_key(&id) {
            self.pending = None;
            return (None, self.reply(&command, id, Some("ENOENT:image")));
        }
        if let Some(refused) = self.refused(&command) {
            self.pending = None;
            return (None, self.reply(&command, id, Some(refused)));
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
        held.quiet = command.quiet;
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
        held.x = held.x.max(command.x);
        held.y = held.y.max(command.y);
        held.source_width = held.source_width.max(command.source_width);
        held.source_height = held.source_height.max(command.source_height);
        held.offset_x = held.offset_x.max(command.offset_x);
        held.offset_y = held.offset_y.max(command.offset_y);
        held.number = held.number.max(command.number);
        if command.z != 0 {
            held.z = command.z;
        }
        if command.cursor_movement == CursorMovement::None {
            held.cursor_movement = CursorMovement::None;
        }
        held.unicode |= command.unicode;
        held.parent_image = held.parent_image.max(command.parent_image);
        held.parent_placement = held.parent_placement.max(command.parent_placement);
        if command.parent_offset_x != 0 {
            held.parent_offset_x = command.parent_offset_x;
        }
        if command.parent_offset_y != 0 {
            held.parent_offset_y = command.parent_offset_y;
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
        let payload = std::mem::take(&mut held.payload);
        held.payload = match load(&held, payload) {
            Ok(payload) => payload,
            Err(error) => return (None, self.reply(&command, id, Some(error))),
        };
        if held.action == Action::Frame {
            let outcome = self.frame(id, &held);
            return (None, self.reply_to(&command, id, outcome));
        }
        let display = (held.action == Action::Display).then(|| Display::of(id, &held));
        let image = Image::still(held.format, held.width, held.height, held.payload);
        if !matches!(image.format, Format::Png) && !raw_fits(&image) {
            return (None, self.reply(&command, id, Some("EINVAL:dimensions")));
        }
        self.insert(id, image);
        if held.number != 0 {
            self.numbers.insert(held.number, id);
        }
        (display.map(Effect::Display), self.reply(&command, id, None))
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
    /// An image of one frame.
    pub fn still(format: Format, width: u32, height: u32, bytes: Vec<u8>) -> Self {
        Self {
            format,
            width,
            height,
            bytes,
            frames: Vec::new(),
            gaps: Vec::new(),
            animation: Animation::default(),
            revision: 0,
            frame_revisions: Vec::new(),
        }
    }

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

/// A transmission's bytes, read from the medium its `payload` names and
/// inflated.
pub(super) fn load(command: &Command, payload: Vec<u8>) -> Result<Vec<u8>, &'static str> {
    let mut payload = if command.medium == 'd' {
        payload
    } else {
        let span = crate::media::Span {
            offset: command.read_offset as u64,
            len: command.read_size as u64,
        };
        crate::media::read(command.medium, &payload, span, MAX_IMAGE)
            .map_err(|_| "EBADF:Failed to read image file")?
    };
    // The chunks are one zlib stream between them, so it is inflated whole,
    // and held to the same ceiling as an uncompressed payload.
    if command.compression == Some('z') {
        use miniz_oxide::inflate::{TINFLStatus, decompress_to_vec_zlib_with_limit};
        payload = match decompress_to_vec_zlib_with_limit(&payload, MAX_IMAGE) {
            Ok(inflated) => inflated,
            Err(error) if error.status == TINFLStatus::HasMoreOutput => {
                return Err("EFBIG:payload");
            }
            Err(_) => return Err("EINVAL:compression"),
        };
    }
    Ok(payload)
}

/// A PNG's dimensions, off the `IHDR` that opens every one of them: 8 bytes of
/// signature, a 4-byte length, the `IHDR` tag, then width and height.
///
/// Read here rather than decoded, because the size is what the *grid* needs —
/// how many cells the image will cover, and so where the cursor lands after
/// it. Decoding is the view's, a frame later and only if the image is on
/// screen.
pub(super) fn png_size(bytes: &[u8]) -> Option<(u32, u32)> {
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
pub(super) fn raw_fits(image: &Image) -> bool {
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
