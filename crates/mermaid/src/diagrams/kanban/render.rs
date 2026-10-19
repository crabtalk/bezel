//! Pele's `diagrams/kanban/render.ts`.

use super::db::{KanbanModel, KanbanNode};
use crate::{
    Config, Link, Options, Rendered,
    config::Value,
    diagrams::common::{fit_width::title_room, icon::icon_svg},
    js,
    svg::{
        builder::{esc, esc_text, label_svg, num, text_style},
        root::{Accessible, svg_document},
        theme::{CRITICAL, RADIUS, class_names},
    },
    text::{Label, Measure, Span, Style, icon_markup, layout_label},
    util::url::{link_url, rel_attr, safe_url},
};
use std::fmt::Write as _;

const COLUMN_GAP: f64 = 12.0;
const COLUMN_PAD: f64 = 8.0;
const CARD_PAD: f64 = 10.0;
const CARD_GAP: f64 = 8.0;
const META_GAP: f64 = 4.0;
const ICON_GAP: f64 = 6.0;

/// Mermaid names five priorities and gives Medium no color. Matching here
/// ignores case. The high ones are marked in the critical color and the low
/// ones in the muted one, the nearer to Medium the fainter. They are not
/// series colors, as a theme may put those in any order.
fn priority_color(priority: &str) -> Option<String> {
    Some(match priority {
        "very high" => format!("stroke=\"{CRITICAL}\""),
        "high" => format!("stroke=\"{CRITICAL}\" opacity=\"0.5\""),
        "low" => "stroke=\"var(--_m)\" opacity=\"0.5\"".into(),
        "very low" => "stroke=\"var(--_m)\"".into(),
        _ => return None,
    })
}

/// Lays out text for a fixed width. A word too long for a line is broken
/// between characters, because a card cannot grow sideways to hold it.
fn fit_label(
    raw: Option<&str>,
    markdown: bool,
    measure: &dyn Measure,
    size: f64,
    max: f64,
    base: u8,
) -> Label {
    let label = layout_label(raw, markdown, measure, size, max, base);
    if label.width <= max {
        return label;
    }
    let mut lines: Vec<Vec<Span>> = Vec::new();
    let mut widths: Vec<f64> = Vec::new();
    let mut widest: f64 = 0.0;
    let mut line: Vec<Span> = Vec::new();
    let mut width = 0.0;
    let mut commit = |line: &mut Vec<Span>, width: &mut f64| {
        lines.push(std::mem::take(line));
        widths.push(*width);
        widest = widest.max(*width);
        *width = 0.0;
    };
    for i in 0..label.lines.len() {
        if label.widths[i] <= max {
            line = label.lines[i].clone();
            width = label.widths[i];
            commit(&mut line, &mut width);
            continue;
        }
        for span in &label.lines[i] {
            if span.icon.is_some() {
                if width > 0.0 && width + span.width > max {
                    commit(&mut line, &mut width);
                }
                line.push(span.clone());
                width += span.width;
                continue;
            }
            let mut text = String::new();
            let mut from = width;
            for ch in span.text.chars() {
                let mut buf = [0u8; 4];
                let w = measure.width(ch.encode_utf8(&mut buf), size, span.style);
                if width > 0.0 && width + w > max {
                    if !text.is_empty() {
                        line.push(Span {
                            text: std::mem::take(&mut text),
                            style: span.style,
                            icon: None,
                            width: width - from,
                        });
                    }
                    commit(&mut line, &mut width);
                    from = 0.0;
                }
                text.push(ch);
                width += w;
            }
            if !text.is_empty() {
                line.push(Span {
                    text,
                    style: span.style,
                    icon: None,
                    width: width - from,
                });
            }
        }
        commit(&mut line, &mut width);
    }
    let height = lines.len() as f64 * label.line_height;
    Label {
        lines,
        widths,
        width: widest,
        height,
        ..label
    }
}

/// Writes a label with every line starting at x, or ending there, and the
/// first line's box starting at y.
fn text_block(label: &Label, x: f64, y: f64, attrs: &str, end: bool) -> String {
    let mut out = String::new();
    for (i, line) in label.lines.iter().enumerate() {
        let cy = y + (i as f64 + 0.5) * label.line_height;
        if line.len() == 1 && line[0].icon.is_none() {
            let _ = write!(
                out,
                "<text{} x=\"{}\" y=\"{}\"{}>{}</text>",
                text_style(attrs, line[0].style),
                num(x),
                num(cy + label.size * 0.35),
                if end { " text-anchor=\"end\"" } else { "" },
                esc(&line[0].text)
            );
        } else {
            let width = label.widths[i];
            let one = Label {
                lines: vec![line.clone()],
                widths: vec![width],
                width,
                height: label.line_height,
                ..label.clone()
            };
            out += &label_svg(
                &one,
                x + (if end { -width } else { width }) / 2.0,
                cy,
                attrs,
            );
        }
    }
    out
}

struct Card<'a> {
    item: &'a KanbanNode,
    label: Label,
    ticket: Label,
    assigned: Label,
    /// Height of the rows under the text that hold the ticket and the
    /// assignee.
    meta: f64,
    height: f64,
}

fn has(name: Option<&str>) -> bool {
    name.is_some_and(|name| !name.is_empty() && !icon_markup(name).is_empty())
}

pub fn render_kanban(model: &KanbanModel, config: &Config, options: &Options) -> Rendered {
    let size = options.font_size.unwrap_or(16.0);
    let small = js::round(size * 0.875);
    let line = js::round(size * 1.5);
    let small_line = js::round(small * 1.5);
    let measure = options.measurer();
    let measure = &*measure;
    let pad = options.padding.unwrap_or(8.0);
    let p = crate::PREFIX;
    let conf = config.section("kanban");
    let own_width = match conf.and_then(|conf| conf.get("sectionWidth")) {
        Some(Value::Number(width)) if *width > 0.0 => width.clamp(80.0, 2000.0),
        _ => 200.0,
    };
    // A board wider than the width the host has is folded into rows of
    // columns, which then share the row between them.
    let room = match options.max_width {
        Some(max) if max > 0.0 => max - 2.0 * pad,
        _ => f64::INFINITY,
    };
    let count = model.sections.len();
    let fits = count as f64 * (own_width + COLUMN_GAP) - COLUMN_GAP <= room;
    let per_row = if fits {
        count.max(1)
    } else {
        1f64.max(((room + COLUMN_GAP) / (own_width + COLUMN_GAP)).floor()) as usize
    };
    let column_width = if fits {
        own_width
    } else {
        own_width
            .min(room)
            .max((((room + COLUMN_GAP) / per_row as f64).floor() - COLUMN_GAP).min(own_width * 2.0))
    };
    let base_url = match conf.and_then(|conf| conf.get("ticketBaseUrl")) {
        Some(Value::String(url)) => url.as_str(),
        _ => "",
    };
    let card_width = column_width - 2.0 * COLUMN_PAD;
    let text_width = card_width - 2.0 * CARD_PAD;
    let glyph = small * 1.25;
    let mut links: Vec<Link> = Vec::new();

    let sections = &model.sections;
    let mut titles: Vec<Label> = Vec::new();
    let mut count_widths: Vec<f64> = Vec::new();
    let mut cards: Vec<Vec<Card>> = Vec::new();
    let mut head_height = line;
    let mut body_heights: Vec<f64> = Vec::new();
    for section in sections {
        let count_width = measure.width(&section.items.len().to_string(), small, 0);
        let reserved = count_width
            + 12.0
            + if has(section.node.icon.as_deref()) {
                glyph + ICON_GAP
            } else {
                0.0
            };
        let title = fit_label(
            Some(&section.node.label),
            true,
            measure,
            size,
            card_width - 8.0 - reserved,
            Style::BOLD,
        );
        head_height = head_height.max(title.height);
        count_widths.push(count_width);
        titles.push(title);

        let mut list: Vec<Card> = Vec::new();
        let mut height = 0.0;
        for item in &section.items {
            let label = fit_label(
                Some(&item.label),
                true,
                measure,
                small,
                text_width
                    - if has(item.icon.as_deref()) {
                        glyph + ICON_GAP
                    } else {
                        0.0
                    },
                0,
            );
            let ticket = fit_label(item.ticket.as_deref(), false, measure, small, text_width, 0);
            let assigned = fit_label(
                item.assigned.as_deref(),
                false,
                measure,
                small,
                text_width,
                0,
            );
            let meta = if ticket.width + assigned.width + 8.0 > text_width {
                ticket.height + assigned.height
            } else {
                ticket.height.max(assigned.height)
            };
            let card_height = 2.0 * CARD_PAD
                + label.height.max(small_line)
                + if js::truthy(meta) {
                    META_GAP + meta
                } else {
                    0.0
                };
            list.push(Card {
                item,
                label,
                ticket,
                assigned,
                meta,
                height: card_height,
            });
            height += card_height + CARD_GAP;
        }
        cards.push(list);
        body_heights.push(height);
    }
    head_height += 2.0 * COLUMN_PAD;
    let mut row_heights: Vec<f64> = Vec::new();
    let mut row_tops: Vec<f64> = Vec::new();
    let mut first = 0;
    let mut y = 0.0;
    while first < sections.len() {
        let mut tallest = CARD_GAP;
        for height in &body_heights[first..(first + per_row).min(sections.len())] {
            tallest = tallest.max(*height);
        }
        row_tops.push(y);
        row_heights.push(head_height + tallest);
        y += head_height + tallest + COLUMN_GAP;
        first += per_row;
    }
    let board_height = match (row_tops.last(), row_heights.last()) {
        (Some(top), Some(height)) => top + height,
        _ => 0.0,
    };

    let title = layout_label(
        model.title.as_deref(),
        false,
        measure,
        size,
        title_room(options),
        Style::BOLD,
    );
    let title_height = if title.height > 0.0 {
        title.height + 16.0
    } else {
        0.0
    };
    let board_width =
        (sections.len().min(per_row) as f64 * (column_width + COLUMN_GAP) - COLUMN_GAP).max(0.0);
    let width = board_width.max(title.width);
    let left = pad + (width - board_width) / 2.0;
    let edge = column_width - COLUMN_PAD - 4.0;

    let mut out = String::new();
    for (index, section) in sections.iter().enumerate() {
        let row = index / per_row;
        let mut body = format!(
            "<rect width=\"{}\" height=\"{}\" rx=\"{RADIUS}\" fill=\"var(--_s)\"/>",
            js::num_str(column_width),
            num(row_heights[row])
        );
        body += &text_block(
            &titles[index],
            COLUMN_PAD + 4.0,
            COLUMN_PAD,
            &format!(" class=\"{p}-cluster-label\" font-weight=\"var(--_hw)\""),
            false,
        );
        let _ = write!(
            body,
            "<text class=\"{p}-count\" x=\"{}\" y=\"{}\" text-anchor=\"end\" font-size=\"{}\" fill=\"var(--_m)\">{}</text>",
            js::num_str(edge),
            num(COLUMN_PAD + line / 2.0 + small * 0.35),
            js::num_str(small),
            section.items.len()
        );
        if let Some(icon) = section.node.icon.as_deref().filter(|icon| has(Some(icon))) {
            body += &icon_svg(
                icon,
                edge - count_widths[index] - ICON_GAP - glyph,
                COLUMN_PAD + (line - glyph) / 2.0,
                glyph,
            );
        }

        let mut y = head_height;
        for card in &cards[index] {
            let item = card.item;
            let priority = js::trim(item.priority.as_deref().unwrap_or("")).to_lowercase();
            let color = priority_color(&priority);
            let mut inner = format!(
                "<rect width=\"{}\" height=\"{}\" rx=\"{RADIUS}\" fill=\"var(--_bg)\" stroke=\"var(--_b)\"/>",
                js::num_str(card_width),
                num(card.height)
            );
            if let Some(color) = &color {
                let _ = write!(
                    inner,
                    "<path class=\"{p}-priority\" d=\"M2.5,5V{}\" fill=\"none\" {color} stroke-width=\"3\" stroke-linecap=\"round\"/>",
                    num(card.height - 5.0)
                );
            }
            inner += &text_block(
                &card.label,
                CARD_PAD,
                CARD_PAD,
                &format!(" class=\"{p}-label\""),
                false,
            );
            if let Some(icon) = item.icon.as_deref().filter(|icon| has(Some(icon))) {
                inner += &icon_svg(
                    icon,
                    card_width - CARD_PAD - glyph,
                    CARD_PAD + (small_line - glyph) / 2.0,
                    glyph,
                );
            }
            let bottom = card.height - CARD_PAD;
            if let Some(ticket) = item.ticket.as_deref().filter(|ticket| !ticket.is_empty()) {
                let top = bottom - card.meta;
                let href = if base_url.is_empty() {
                    None
                } else {
                    link_url(
                        &safe_url(&base_url.replacen("#TICKET#", ticket, 1)),
                        options,
                    )
                };
                match href {
                    None => {
                        inner += &text_block(
                            &card.ticket,
                            CARD_PAD,
                            top,
                            &format!(" class=\"{p}-ticket\" fill=\"var(--_m)\""),
                            false,
                        );
                    }
                    Some(href) => {
                        let _ = write!(
                            inner,
                            "<a href=\"{}\" target=\"_blank\"{}>{}</a>",
                            esc(&href),
                            rel_attr(options),
                            text_block(
                                &card.ticket,
                                CARD_PAD,
                                top,
                                &format!(" class=\"{p}-ticket\" fill=\"var(--_c)\""),
                                false,
                            )
                        );
                        links.push(Link {
                            id: item.id.clone(),
                            href,
                            internal: false,
                        });
                    }
                }
            }
            if item
                .assigned
                .as_deref()
                .is_some_and(|assigned| !assigned.is_empty())
            {
                inner += &text_block(
                    &card.assigned,
                    card_width - CARD_PAD,
                    bottom - card.assigned.height,
                    &format!(" class=\"{p}-assigned\" fill=\"var(--_m)\""),
                    true,
                );
            }
            let _ = write!(
                body,
                "<g class=\"{p}-node {p}-card{}{}\" data-id=\"{}\" transform=\"translate({},{})\" font-size=\"{}\">{inner}</g>",
                if color.is_none() {
                    String::new()
                } else {
                    format!(" {p}-priority-{}", priority.replacen(' ', "-", 1))
                },
                class_names(item.css_classes.as_deref().unwrap_or("")),
                esc_text(&item.id),
                js::num_str(COLUMN_PAD),
                num(y),
                js::num_str(small)
            );
            y += card.height + CARD_GAP;
        }

        let _ = write!(
            out,
            "<g class=\"{p}-cluster {p}-column{}\" data-id=\"{}\" transform=\"translate({},{})\">{body}</g>",
            class_names(section.node.css_classes.as_deref().unwrap_or("")),
            esc_text(&section.node.id),
            num(left + (index % per_row) as f64 * (column_width + COLUMN_GAP)),
            num(pad + title_height + row_tops[row])
        );
    }

    let total_width = (width + 2.0 * pad).ceil();
    let total_height = (board_height + title_height + 2.0 * pad).ceil();
    let content = label_svg(
        &title,
        total_width / 2.0,
        pad + title.height / 2.0,
        &format!(" class=\"{p}-title\" font-weight=\"var(--_tw)\""),
    ) + &out;

    Rendered {
        svg: svg_document(
            "kanban",
            total_width,
            total_height,
            size,
            options,
            Accessible::default(),
            &content,
        ),
        width: total_width,
        height: total_height,
        links,
    }
}
