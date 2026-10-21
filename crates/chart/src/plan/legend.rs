//! What each legend lists: the colours, then each shape, dash, size and
//! opacity field's values, a channel over the colour field folding into the
//! colour legend.

use gpui::SharedString;

use crate::{
    model::{Channel, Chart, Encode, Layer, Mark, Shape},
    plan::{
        Glyph, Ink,
        domain::{Colors, Discrete},
        look::{self, Amount},
    },
};

/// A legend's symbols, in square pixels.
const SYMBOL: f32 = 64.0;

type Entries = Vec<(Glyph, SharedString)>;

/// Each legend's title and entries. `None` for a channel over a field its
/// layer lacks.
pub(super) fn legends(
    chart: &Chart,
    colors: &Colors,
) -> Option<Vec<(Option<SharedString>, Entries)>> {
    let scaled = |pick: fn(&Layer) -> Option<&Channel>| {
        chart
            .layers
            .iter()
            .find_map(|layer| Some((layer, pick(layer)?)))
    };
    let color = scaled(|l| l.encoding.color.as_ref()?.channel());
    let color_field = color.and_then(|(_, channel)| channel.field());
    let own = |channel: &Channel| color_field.is_none() || channel.field() != color_field;

    let mut legends = Vec::new();
    if let Some((layer, channel)) = color {
        let shapes = match layer.encoding.shape.as_ref().and_then(Encode::channel) {
            Some(shape) if !own(shape) => Some(values(layer, shape, Shape::nth)?),
            _ => None,
        };
        let dashes = match layer
            .encoding
            .stroke_dash
            .as_ref()
            .and_then(Encode::channel)
        {
            Some(dash) if !own(dash) => Some(values(layer, dash, look::dash)?),
            _ => None,
        };
        let entries = colors
            .colors
            .iter()
            .filter_map(|color| {
                let name = color.name.clone()?;
                let glyph = match (&shapes, &dashes) {
                    (Some(shapes), _) => Glyph::Symbol {
                        shape: shapes
                            .iter()
                            .find(|(label, _)| *label == name)
                            .map_or(Shape::Circle, |(_, shape)| *shape),
                        size: SYMBOL,
                        ink: color.ink,
                    },
                    (None, Some(dashes)) => Glyph::Stroke {
                        ink: color.ink,
                        width: look::LINE_WIDTH,
                        dash: dashes
                            .iter()
                            .find(|(label, _)| *label == name)
                            .and_then(|(_, dash)| dash.clone()),
                    },
                    (None, None) => Glyph::Fill {
                        ink: color.ink,
                        opacity: 1.0,
                    },
                };
                Some((glyph, name))
            })
            .collect();
        legends.push((channel.title.clone(), entries));
    }

    for pick in [
        (|l: &Layer| l.encoding.fill.as_ref()?.channel()) as fn(&Layer) -> Option<&Channel>,
        |l| l.encoding.stroke.as_ref()?.channel(),
    ] {
        if let Some((layer, channel)) = scaled(pick).filter(|(_, c)| own(c)) {
            let entries = values(layer, channel, Ink::Category)?
                .into_iter()
                .map(|(label, ink)| (Glyph::Fill { ink, opacity: 1.0 }, label))
                .collect();
            legends.push((channel.title.clone(), entries));
        }
    }
    if let Some((layer, channel)) =
        scaled(|l| l.encoding.shape.as_ref()?.channel()).filter(|(_, c)| own(c))
    {
        let entries = values(layer, channel, Shape::nth)?
            .into_iter()
            .map(|(label, shape)| {
                let glyph = Glyph::Symbol {
                    shape,
                    size: SYMBOL,
                    ink: Ink::Muted,
                };
                (glyph, label)
            })
            .collect();
        legends.push((channel.title.clone(), entries));
    }
    if let Some((layer, channel)) =
        scaled(|l| l.encoding.stroke_dash.as_ref()?.channel()).filter(|(_, c)| own(c))
    {
        let entries = values(layer, channel, look::dash)?
            .into_iter()
            .map(|(label, dash)| {
                let glyph = Glyph::Stroke {
                    ink: Ink::Muted,
                    width: look::LINE_WIDTH,
                    dash,
                };
                (glyph, label)
            })
            .collect();
        legends.push((channel.title.clone(), entries));
    }
    if let Some((layer, channel)) = scaled(|l| match l.mark {
        Mark::Point
        | Mark::Circle
        | Mark::Square
        | Mark::Line
        | Mark::Area
        | Mark::Rule
        | Mark::Trail => l.encoding.size.as_ref()?.channel(),
        _ => None,
    }) {
        let range = match layer.mark.is_symbol() {
            true => look::POINT_SIZE,
            false => look::STROKE_WIDTH,
        };
        let amount = Amount::of(channel, &channel.column(&layer.data)?, range, true)?;
        let entries = amount
            .entries()
            .into_iter()
            .map(|(label, size)| {
                let glyph = match layer.mark.is_symbol() {
                    true => Glyph::Symbol {
                        shape: match layer.mark {
                            Mark::Square => Shape::Square,
                            _ => Shape::Circle,
                        },
                        size,
                        ink: Ink::Muted,
                    },
                    false => Glyph::Stroke {
                        ink: Ink::Muted,
                        width: size,
                        dash: None,
                    },
                };
                (glyph, label)
            })
            .collect();
        legends.push((channel.title.clone(), entries));
    }
    if let Some((layer, channel)) = scaled(|l| l.encoding.opacity.as_ref()?.channel()) {
        let amount = Amount::of(channel, &channel.column(&layer.data)?, look::OPACITY, false)?;
        let entries = amount
            .entries()
            .into_iter()
            .map(|(label, opacity)| {
                let glyph = Glyph::Fill {
                    ink: Ink::Muted,
                    opacity,
                };
                (glyph, label)
            })
            .collect();
        legends.push((channel.title.clone(), entries));
    }
    Some(legends)
}

/// A discrete field's values in order, each with `scheme`'s value at its
/// place, `scheme` taken in turn.
fn values<T: Clone>(
    layer: &Layer,
    channel: &Channel,
    scheme: impl Fn(usize) -> T,
) -> Option<Vec<(SharedString, T)>> {
    if !channel.kind.is_discrete() {
        return None;
    }
    let domain = Discrete::of(&channel.column(&layer.data)?, &channel.sort);
    Some(
        domain
            .labels
            .into_iter()
            .enumerate()
            .map(|(i, label)| (label, scheme(i)))
            .collect(),
    )
}
