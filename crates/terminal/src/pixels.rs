//! RGBA buffers for animation frames: decoding what a client sent into one,
//! and drawing one rectangle of pixels over another.

use crate::kitty::Format;

/// Pixels, four bytes each, row after row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rgba {
    pub width: u32,
    pub height: u32,
    pub bytes: Vec<u8>,
    /// Every pixel is opaque, which is what RGB data always is. Opaque data
    /// replaces what it lands on rather than blending with it.
    pub opaque: bool,
}

impl Rgba {
    /// A buffer of one color, `0xRRGGBBAA`.
    pub fn filled(width: u32, height: u32, color: u32) -> Self {
        let pixel = color.to_be_bytes();
        Self {
            width,
            height,
            bytes: pixel.repeat(width as usize * height as usize),
            opaque: pixel[3] == 0xff,
        }
    }

    /// `bytes` in `format`, with the size a raw format states. `None` for a
    /// payload that does not decode or holds fewer pixels than it claims.
    pub fn decode(format: Format, width: u32, height: u32, bytes: &[u8]) -> Option<Self> {
        match format {
            Format::Png => {
                let decoded = image::load_from_memory(bytes).ok()?.to_rgba8();
                Some(Self {
                    width: decoded.width(),
                    height: decoded.height(),
                    bytes: decoded.into_raw(),
                    opaque: false,
                })
            }
            Format::Rgb | Format::Rgba => {
                let pixels = (width as usize).checked_mul(height as usize)?;
                if pixels == 0 {
                    return None;
                }
                let channels = if format == Format::Rgb { 3 } else { 4 };
                let data = bytes.get(..pixels.checked_mul(channels)?)?;
                let bytes = match format {
                    Format::Rgb => data
                        .as_chunks::<3>()
                        .0
                        .iter()
                        .flat_map(|pixel| [pixel[0], pixel[1], pixel[2], 0xff])
                        .collect(),
                    _ => data.to_vec(),
                };
                Some(Self {
                    width,
                    height,
                    bytes,
                    opaque: format == Format::Rgb,
                })
            }
        }
    }
}

/// A rectangle in pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rect {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

impl Rect {
    pub fn overlaps(&self, other: &Rect) -> bool {
        self.x < other.x + other.width
            && other.x < self.x + self.width
            && self.y < other.y + other.height
            && other.y < self.y + self.height
    }

    /// Whether it lies inside a `width` by `height` buffer.
    pub fn within(&self, width: u32, height: u32) -> bool {
        self.x
            .checked_add(self.width)
            .is_some_and(|right| right <= width)
            && self
                .y
                .checked_add(self.height)
                .is_some_and(|bottom| bottom <= height)
    }
}

/// Draw `from`'s pixels in `source` over `onto`'s at `(x, y)`, clipped to
/// `onto`. `replace` copies them; otherwise they are alpha-blended over what
/// is there.
pub fn draw(onto: &mut Rgba, x: u32, y: u32, from: &Rgba, source: Rect, replace: bool) {
    let width = source
        .width
        .min(onto.width.saturating_sub(x))
        .min(from.width.saturating_sub(source.x));
    let height = source
        .height
        .min(onto.height.saturating_sub(y))
        .min(from.height.saturating_sub(source.y));
    for row in 0..height {
        for col in 0..width {
            let over =
                4 * ((source.y + row) as usize * from.width as usize + (source.x + col) as usize);
            let under = 4 * ((y + row) as usize * onto.width as usize + (x + col) as usize);
            let over: [u8; 4] = from.bytes[over..over + 4].try_into().unwrap();
            let under_pixel = &mut onto.bytes[under..under + 4];
            if replace || over[3] == 0xff {
                under_pixel.copy_from_slice(&over);
            } else {
                blend(under_pixel, over);
            }
        }
    }
    if !replace || !from.opaque {
        onto.opaque = false;
    }
}

/// `over` composited onto `under` in place, straight alpha.
fn blend(under: &mut [u8], over: [u8; 4]) {
    let over_alpha = over[3] as f32 / 255.0;
    let under_alpha = under[3] as f32 / 255.0;
    let alpha = over_alpha + under_alpha * (1.0 - over_alpha);
    if alpha <= 0.0 {
        under.copy_from_slice(&[0, 0, 0, 0]);
        return;
    }
    for channel in 0..3 {
        let value = (over[channel] as f32 * over_alpha
            + under[channel] as f32 * under_alpha * (1.0 - over_alpha))
            / alpha;
        under[channel] = value.round().clamp(0.0, 255.0) as u8;
    }
    under[3] = (alpha * 255.0).round() as u8;
}
