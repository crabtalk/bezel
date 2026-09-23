//! iTerm2's inline images: `OSC 1337 ; File=<args> : <base64>`, and the same
//! file sent in parts between `MultipartFile=<args>` and `FileEnd`.
//!
//! What an image decodes to is held in the kitty store and placed the way
//! kitty's `a=T` places one.

use crate::kitty::{Animation, AnimationState, Format, Image};

/// The most one file may hold, decoded.
pub const MAX_FILE: usize = 64 << 20;

/// The most an animated GIF's frames may hold between them, decoded.
const MAX_FRAMES: usize = 256 << 20;

/// A file's arguments, as far as display goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Args {
    /// `inline=1`. Anything else is a download, which is not shown.
    pub inline: bool,
    pub width: Size,
    pub height: Size,
    /// `preserveAspectRatio`, on unless it is `0`.
    pub preserve_aspect: bool,
}

/// A `width` or `height`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Size {
    /// `auto`, or not given: the image's own.
    #[default]
    Auto,
    Cells(u32),
    Pixels(u32),
    Percent(u32),
}

impl Args {
    /// `key=value` pairs separated by `;`. Unknown keys and malformed values
    /// are left at their defaults.
    pub fn parse(args: &[u8]) -> Self {
        let mut out = Self {
            preserve_aspect: true,
            ..Self::default()
        };
        let args = String::from_utf8_lossy(args);
        for pair in args.split(';') {
            let Some((key, value)) = pair.split_once('=') else {
                continue;
            };
            match key {
                "inline" => out.inline = value == "1",
                "width" => out.width = Size::parse(value),
                "height" => out.height = Size::parse(value),
                "preserveAspectRatio" => out.preserve_aspect = value != "0",
                _ => {}
            }
        }
        out
    }
}

impl Size {
    fn parse(value: &str) -> Self {
        let number = |digits: &str| digits.parse::<u32>().ok();
        if let Some(pixels) = value.strip_suffix("px").and_then(number) {
            Size::Pixels(pixels)
        } else if let Some(percent) = value.strip_suffix('%').and_then(number) {
            Size::Percent(percent)
        } else {
            number(value).map_or(Size::Auto, Size::Cells)
        }
    }

    /// The size in cells, given one cell's pixels along this axis and the
    /// grid's cells along it. Zero is the image's own.
    pub fn cells(self, cell: f32, grid: usize) -> u32 {
        match self {
            Size::Auto => 0,
            Size::Cells(cells) => cells,
            Size::Pixels(pixels) => (pixels as f32 / cell).ceil() as u32,
            Size::Percent(percent) => (grid as f32 * percent.min(100) as f32 / 100.0).ceil() as u32,
        }
    }
}

/// A file's bytes as an image. An animated GIF keeps its frames and plays
/// them, looping.
pub fn image(bytes: &[u8]) -> Option<Image> {
    if image::guess_format(bytes).ok()? == image::ImageFormat::Gif
        && let Some(animated) = gif(bytes)
    {
        return Some(animated);
    }
    let rgba = image::load_from_memory(bytes).ok()?.to_rgba8();
    Some(Image::still(
        Format::Rgba,
        rgba.width(),
        rgba.height(),
        rgba.into_raw(),
    ))
}

/// A GIF of more than one frame, every frame composed to the full canvas.
fn gif(bytes: &[u8]) -> Option<Image> {
    use image::AnimationDecoder;
    let decoder = image::codecs::gif::GifDecoder::new(std::io::Cursor::new(bytes)).ok()?;
    let mut frames = Vec::new();
    let mut gaps = Vec::new();
    let mut held = 0;
    for frame in decoder.into_frames() {
        let frame = frame.ok()?;
        let (numer, denom) = frame.delay().numer_denom_ms();
        // A GIF's zero delay is shown as 100ms the way browsers show it.
        let gap = match numer.checked_div(denom).unwrap_or(0) {
            0 => 100,
            gap => gap,
        };
        let buffer = frame.into_buffer();
        held += buffer.len();
        if held > MAX_FRAMES {
            break;
        }
        gaps.push(gap);
        frames.push(buffer);
    }
    if frames.len() < 2 {
        return None;
    }
    let first = frames.remove(0);
    Some(Image {
        frames: frames.into_iter().map(|frame| frame.into_raw()).collect(),
        gaps,
        animation: Animation {
            state: AnimationState::Running,
            ..Animation::default()
        },
        ..Image::still(
            Format::Rgba,
            first.width(),
            first.height(),
            first.into_raw(),
        )
    })
}
