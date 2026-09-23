//! Sixel images: the data of a `DCS P1;P2;P3 q … ST`, decoded to RGBA.
//!
//! Pixels no sixel sets are transparent, whatever `P2` asks for. `P1`'s pixel
//! aspect ratio is not applied: every pixel is square.

use crate::pixels::Rgba;

/// The widest and tallest image decoded.
const MAX_SIDE: u32 = 10_000;

/// The most one decoded image may hold, in bytes.
const MAX_BYTES: usize = 64 << 20;

/// The VT340's sixteen colors, in percent, which the color registers hold
/// until a program sets them.
const VT340: [(u32, u32, u32); 16] = [
    (0, 0, 0),
    (20, 20, 80),
    (80, 13, 13),
    (20, 80, 20),
    (80, 20, 80),
    (20, 80, 80),
    (80, 80, 20),
    (53, 53, 53),
    (26, 26, 26),
    (33, 33, 60),
    (60, 26, 26),
    (33, 60, 33),
    (60, 33, 60),
    (33, 60, 60),
    (60, 60, 33),
    (80, 80, 80),
];

/// The image `data` draws. `None` for one that draws nothing, or one past
/// [`MAX_SIDE`] or [`MAX_BYTES`].
pub fn decode(data: &[u8]) -> Option<Rgba> {
    let mut width = 0u32;
    let mut height = 0u32;
    let raster = walk(data, |x, y, _| {
        width = width.max(x + 1);
        height = height.max(y + 1);
    });
    if let Some((raster_width, raster_height)) = raster {
        width = width.max(raster_width);
        height = height.max(raster_height);
    }
    if width == 0 || height == 0 || width > MAX_SIDE || height > MAX_SIDE {
        return None;
    }
    let len = (width as usize * height as usize).checked_mul(4)?;
    if len > MAX_BYTES {
        return None;
    }
    let mut bytes = vec![0u8; len];
    walk(data, |x, y, color| {
        let at = 4 * (y as usize * width as usize + x as usize);
        bytes[at..at + 4].copy_from_slice(&color);
    });
    Some(Rgba {
        width,
        height,
        bytes,
        opaque: false,
    })
}

/// Run `data`'s commands, calling `plot` for every pixel a sixel sets.
/// Answers the raster attributes' size, if the data states one.
fn walk(data: &[u8], mut plot: impl FnMut(u32, u32, [u8; 4])) -> Option<(u32, u32)> {
    let mut palette = [[0u8, 0, 0, 0xff]; 256];
    for (register, &(r, g, b)) in VT340.iter().enumerate() {
        palette[register] = percent(r, g, b);
    }
    let mut color = palette[0];
    let mut raster = None;
    let (mut x, mut y) = (0u32, 0u32);
    let mut at = 0;
    while at < data.len() {
        let byte = data[at];
        at += 1;
        match byte {
            b'"' => {
                let numbers = numbers(data, &mut at);
                if let [_, _, width, height, ..] = numbers[..] {
                    raster = Some((width, height));
                }
            }
            b'#' => {
                let numbers = numbers(data, &mut at);
                let Some(&register) = numbers.first() else {
                    continue;
                };
                let register = (register as usize).min(255);
                match numbers[..] {
                    [_, 1, h, l, s, ..] => palette[register] = hls(h, l, s),
                    [_, 2, r, g, b, ..] => palette[register] = percent(r, g, b),
                    _ => {}
                }
                color = palette[register];
            }
            b'!' => {
                let count = numbers(data, &mut at).first().copied().unwrap_or(1).max(1);
                if let Some(&sixel @ b'?'..=b'~') = data.get(at) {
                    at += 1;
                    for _ in 0..count.min(MAX_SIDE) {
                        column(sixel, x, y, color, &mut plot);
                        x = x.saturating_add(1);
                    }
                }
            }
            b'$' => x = 0,
            b'-' => {
                x = 0;
                y = y.saturating_add(6);
            }
            b'?'..=b'~' => {
                column(byte, x, y, color, &mut plot);
                x = x.saturating_add(1);
            }
            _ => {}
        }
        // Past the largest image decoded, nothing more is worth walking.
        if x > MAX_SIDE || y > MAX_SIDE {
            break;
        }
    }
    raster
}

/// One sixel: six pixels down from `(x, y)`, one per bit, low bit on top.
fn column(sixel: u8, x: u32, y: u32, color: [u8; 4], plot: &mut impl FnMut(u32, u32, [u8; 4])) {
    let bits = sixel - b'?';
    for bit in 0..6 {
        if bits & (1 << bit) != 0 {
            plot(x, y + bit, color);
        }
    }
}

/// The `;`-separated numbers from `at`, leaving `at` on the byte after.
/// A missing number is zero.
fn numbers(data: &[u8], at: &mut usize) -> Vec<u32> {
    let mut out = vec![0u32];
    while let Some(&byte) = data.get(*at) {
        match byte {
            b'0'..=b'9' => {
                let last = out.last_mut().unwrap();
                *last = last.saturating_mul(10).saturating_add((byte - b'0') as u32);
            }
            b';' => out.push(0),
            _ => break,
        }
        *at += 1;
    }
    out
}

fn percent(r: u32, g: u32, b: u32) -> [u8; 4] {
    let channel = |value: u32| ((value.min(100) * 255 + 50) / 100) as u8;
    [channel(r), channel(g), channel(b), 0xff]
}

/// A DEC HLS color. DEC puts blue at 0°, red at 120° and green at 240°.
fn hls(hue: u32, lightness: u32, saturation: u32) -> [u8; 4] {
    let hue = ((hue % 360 + 240) % 360) as f32 / 360.0;
    let lightness = lightness.min(100) as f32 / 100.0;
    let saturation = saturation.min(100) as f32 / 100.0;
    if saturation == 0.0 {
        let gray = (lightness * 255.0).round() as u8;
        return [gray, gray, gray, 0xff];
    }
    let q = if lightness < 0.5 {
        lightness * (1.0 + saturation)
    } else {
        lightness + saturation - lightness * saturation
    };
    let p = 2.0 * lightness - q;
    let channel = |t: f32| {
        let t = t.rem_euclid(1.0);
        let value = if t < 1.0 / 6.0 {
            p + (q - p) * 6.0 * t
        } else if t < 0.5 {
            q
        } else if t < 2.0 / 3.0 {
            p + (q - p) * (2.0 / 3.0 - t) * 6.0
        } else {
            p
        };
        (value * 255.0).round() as u8
    };
    [
        channel(hue + 1.0 / 3.0),
        channel(hue),
        channel(hue - 1.0 / 3.0),
        0xff,
    ]
}
