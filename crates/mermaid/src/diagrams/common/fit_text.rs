//! One line of text fitted into a width. Pele's
//! `diagrams/common/fit-text.ts`.

use crate::{js, text::Host};

pub struct Fitted {
    pub text: String,
    pub size: f64,
    pub width: f64,
}

/// Fits `text` into `max`: first by shrinking the font down to `min`, then
/// by cutting the text and adding an ellipsis. `None` when not even one
/// character fits.
pub fn fit_text(
    text: &str,
    max: f64,
    size: f64,
    min: f64,
    measure: &impl Host,
    style: u8,
) -> Option<Fitted> {
    if text.is_empty() || max <= 0.0 {
        return None;
    }
    let mut width = measure.width(text, size, style);
    if width <= max {
        return Some(Fitted {
            text: text.to_string(),
            size,
            width,
        });
    }

    // Width grows with font size, so one division lands on the right size or
    // just above it.
    let mut fitted = min.max((size * max / width).floor());
    while fitted >= min {
        width = measure.width(text, fitted, style);
        if width <= max {
            return Some(Fitted {
                text: text.to_string(),
                size: fitted,
                width,
            });
        }
        fitted -= 1.0;
    }

    let chars: Vec<char> = text.chars().collect();
    let cut_at = |n: usize| -> String {
        let head: String = chars[..n].iter().collect();
        format!("{}…", js::trim_end(&head))
    };
    let (mut low, mut high) = (0usize, chars.len().saturating_sub(1));
    while low < high {
        let mid = (low + high + 1) >> 1;
        if measure.width(&cut_at(mid), min, style) <= max {
            low = mid;
        } else {
            high = mid - 1;
        }
    }
    if low == 0 {
        return None;
    }
    let cut = cut_at(low);
    width = measure.width(&cut, min, style);
    (width <= max).then_some(Fitted {
        text: cut,
        size: min,
        width,
    })
}
