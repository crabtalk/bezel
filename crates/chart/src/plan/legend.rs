//! What each legend lists: the colours, then each shape, dash, size and
//! opacity field's values, a channel over the colour field folding into the
//! colour legend.

use gpui::SharedString;

use crate::{
    model::{Channel, Chart, Encode, Layer, Mark, ScaleType, Shape},
    plan::{
        Glyph, Ink,
        domain::{self, Colors, Discrete},
        look::{self, Amount, Dashes, Shapes},
        scales::{self, ColorScale, Colored},
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
            Some(shape) if !own(shape) && shape.scale.kind != Some(ScaleType::Identity) => {
                let range = Shapes::of(shape)?;
                Some(look::listed(shape, layer, |i| range.nth(i), range.len())?)
            }
            _ => None,
        };
        let dashes = match layer
            .encoding
            .stroke_dash
            .as_ref()
            .and_then(Encode::channel)
        {
            Some(dash) if !own(dash) => {
                let range = Dashes::of(dash)?;
                Some(look::listed(dash, layer, |i| range.nth(i), range.len())?)
            }
            _ => None,
        };
        let entries = match &colors.legend {
            Some(entries) => entries
                .iter()
                .map(|(ink, label)| {
                    let glyph = Glyph::Fill {
                        ink: *ink,
                        opacity: 1.0,
                    };
                    (glyph, label.clone())
                })
                .collect(),
            None => colors
                .colors
                .iter()
                .filter_map(|color| {
                    let name = color.name.clone()?;
                    let glyph = match (&shapes, &dashes) {
                        (Some(shapes), _) => Glyph::Symbol {
                            shape: shapes
                                .iter()
                                .find(|(label, _)| *label == name)
                                .map_or(Shape::Circle, |(_, shape)| shape.clone()),
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
                .collect(),
        };
        legends.push((channel.title.clone(), entries));
    }

    for pick in [
        (|l: &Layer| l.encoding.fill.as_ref()?.channel()) as fn(&Layer) -> Option<&Channel>,
        |l| l.encoding.stroke.as_ref()?.channel(),
    ] {
        if let Some((layer, channel)) = scaled(pick).filter(|(_, c)| own(c)) {
            let entries = paints(layer, channel)?
                .into_iter()
                .map(|(ink, label)| (Glyph::Fill { ink, opacity: 1.0 }, label))
                .collect();
            legends.push((channel.title.clone(), entries));
        }
    }
    if let Some((layer, channel)) = scaled(|l| l.encoding.shape.as_ref()?.channel())
        .filter(|(_, c)| own(c) && c.scale.kind != Some(ScaleType::Identity))
    {
        let range = Shapes::of(channel)?;
        let entries = look::listed(channel, layer, |i| range.nth(i), range.len())?
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
        let range = Dashes::of(channel)?;
        let entries = look::listed(channel, layer, |i| range.nth(i), range.len())?
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
        let amount = Amount::of(channel, &layer.data, range, layer.mark, true)?;
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
        let amount = Amount::of(channel, &layer.data, look::OPACITY, layer.mark, false)?;
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

/// A fill or stroke channel's legend: each discrete value in its colour, or
/// a continuous scale's ticks, or each step.
fn paints(layer: &Layer, channel: &Channel) -> Option<Vec<(Ink, SharedString)>> {
    // With no scale, Vega-Lite draws no legend; a range field's colours are
    // each value's.
    if channel.scale.kind == Some(ScaleType::Identity) {
        return Some(Vec::new());
    }
    if let Some(rows) = scales::named_inks(channel, layer)? {
        let domain = Discrete::of(&channel.column(&layer.data)?, &channel.sort);
        let mut entries: Vec<Option<(Ink, SharedString)>> = vec![None; domain.labels.len()];
        for (row, &code) in domain.codes.iter().enumerate() {
            if code != domain::MISSING && entries[code as usize].is_none() {
                entries[code as usize] =
                    rows[row].map(|ink| (ink, domain.labels[code as usize].clone()));
            }
        }
        return Some(entries.into_iter().flatten().collect());
    }
    Some(match ColorScale::over(channel, layer)? {
        (palette, Colored::Discrete(domain)) => domain
            .labels
            .into_iter()
            .enumerate()
            .map(|(i, label)| Some((palette.discrete(i)?, label)))
            .collect::<Option<_>>()?,
        (palette, Colored::Numbers(_)) => palette.entries(channel.kind, domain::LEGEND_TICKS),
    })
}
