//! Shaping and painting: runs, rows, selection and caret.

use super::*;

/// What gets painted, and whether it is the placeholder — which is the only
/// reason the colour differs.
pub(super) fn display_text(field: &TextField) -> (SharedString, bool) {
    if field.content.is_empty() {
        (field.placeholder.clone(), true)
    } else {
        (field.content.clone(), false)
    }
}

/// One run per span, the text between them in the field's own colour.
///
/// The runs a field paints [`TextField::set_spans`] as, public for the same
/// reason [`next_boundary`] is: anything shaping the same text with the same
/// palette wants the same answer, and this is where it is decided.
///
/// The spans are whatever the caller last handed over, which may describe text
/// this frame no longer holds — see [`TextField::set_spans`]. A range that runs
/// past the end, overlaps the one before it, or cuts a character in half is
/// dropped rather than shifting the runs after it: shaping needs the lengths to
/// add up to the text, and a colour that is wrong for one frame is cheaper than
/// a panic.
///
/// `base.len` is ignored: every run is measured off `text`.
pub fn coloured(
    text: &str,
    spans: &[(Range<usize>, HighlightKind)],
    base: &TextRun,
    palette: &SyntaxPalette,
) -> Vec<TextRun> {
    if spans.is_empty() {
        return vec![TextRun {
            len: text.len(),
            ..base.clone()
        }];
    }
    let mut runs = Vec::with_capacity(spans.len() * 2 + 1);
    let mut at = 0;
    for (range, kind) in spans {
        if range.start < at
            || range.end <= range.start
            || range.end > text.len()
            || !text.is_char_boundary(range.start)
            || !text.is_char_boundary(range.end)
        {
            continue;
        }
        if range.start > at {
            runs.push(TextRun {
                len: range.start - at,
                ..base.clone()
            });
        }
        runs.push(TextRun {
            len: range.end - range.start,
            color: palette.color(*kind),
            ..base.clone()
        });
        at = range.end;
    }
    if at < text.len() {
        runs.push(TextRun {
            len: text.len() - at,
            ..base.clone()
        });
    }
    runs
}

/// The IME composition range underlined, so the user can see what is still
/// provisional. Each run is cut at the range's edges and the pieces inside it
/// take the line, in whatever colour they already had.
pub fn underlined(runs: Vec<TextRun>, marked: &Range<usize>) -> Vec<TextRun> {
    let mut out = Vec::with_capacity(runs.len() + 2);
    let mut at = 0;
    for run in runs {
        let end = at + run.len;
        for (start, stop, mark) in [
            (at, end.min(marked.start), false),
            (at.max(marked.start), end.min(marked.end), true),
            (at.max(marked.end), end, false),
        ] {
            if stop <= start {
                continue;
            }
            out.push(TextRun {
                len: stop - start,
                underline: mark.then(|| UnderlineStyle {
                    color: Some(run.color),
                    thickness: px(1.0),
                    wavy: false,
                }),
                ..run.clone()
            });
        }
        at = end;
    }
    out
}

// ---------------------------------------------------------------------------
// Line geometry. `shape_text` returns one `WrappedLine` per hard newline, each
// wrapping into rows of its own; gpui resolves positions *within* a line, so
// everything here is walking that list and nothing re-implements shaping.
// ---------------------------------------------------------------------------

/// Each shaped line with the byte offset it starts at. `shape_text` splits on
/// `\n` and drops the separator, so each line starts one byte past the last.
pub(super) fn lines_from(lines: &[WrappedLine]) -> impl Iterator<Item = (usize, &WrappedLine)> {
    lines.iter().scan(0usize, |start, line| {
        let at = *start;
        *start = at + line.len() + 1;
        Some((at, line))
    })
}

/// Every visual row: the byte range it covers and its top edge, relative to the
/// text origin. A wrap boundary resolves to a byte index exactly the way
/// `WrappedLineLayout::position_for_index` does it internally — that mapping is
/// not exposed, and selection needs one quad per row.
pub(super) fn rows(lines: &[WrappedLine], line_height: Pixels) -> Vec<(Range<usize>, Pixels)> {
    let mut out = Vec::new();
    let mut top = px(0.);
    for (start, line) in lines_from(lines) {
        let mut row_start = start;
        for boundary in line.wrap_boundaries() {
            let at = start + line.runs()[boundary.run_ix].glyphs[boundary.glyph_ix].index;
            out.push((row_start..at, top));
            row_start = at;
            top += line_height;
        }
        out.push((row_start..start + line.len(), top));
        top += line_height;
    }
    out
}

/// Byte offset → position relative to the text origin.
pub(super) fn position_for_offset(
    lines: &[WrappedLine],
    offset: usize,
    line_height: Pixels,
) -> Option<Point<Pixels>> {
    let mut top = px(0.);
    for (start, line) in lines_from(lines) {
        if offset <= start + line.len() {
            let local = line.position_for_index(offset.saturating_sub(start), line_height)?;
            return Some(gpui::point(local.x, local.y + top));
        }
        top += line.size(line_height).height;
    }
    None
}

/// Position relative to the text origin → the closest byte offset.
pub(super) fn offset_for_position(
    lines: &[WrappedLine],
    position: Point<Pixels>,
    line_height: Pixels,
) -> usize {
    let mut top = px(0.);
    let mut last = 0;
    for (start, line) in lines_from(lines) {
        let height = line.size(line_height).height;
        last = start + line.len();
        if position.y < top + height {
            let local = gpui::point(position.x, position.y - top);
            let (Ok(index) | Err(index)) = line.closest_index_for_position(local, line_height);
            return start + index;
        }
        top += height;
    }
    last
}

/// The char boundary at or before `offset`, clamped to the end of `text`.
pub(super) fn floor_boundary(text: &str, offset: usize) -> usize {
    let mut offset = offset.min(text.len());
    while !text.is_char_boundary(offset) {
        offset -= 1;
    }
    offset
}

/// The selection as one rect per visual row, relative to the text origin.
///
/// A row's left edge is always x=0, so a continuation row is taken from there
/// rather than by looking the offset up — at a soft wrap the two rows share a
/// byte offset, and the lookup resolves it to the end of the earlier row.
pub(super) fn selection_rows(
    lines: &[WrappedLine],
    range: &Range<usize>,
    line_height: Pixels,
) -> Vec<Bounds<Pixels>> {
    rows(lines, line_height)
        .into_iter()
        .filter(|(row, _)| range.start <= row.end && range.end >= row.start)
        .filter_map(|(row, top)| {
            let left = if range.start <= row.start {
                px(0.)
            } else {
                position_for_offset(lines, range.start, line_height)?.x
            };
            let right = position_for_offset(lines, range.end.min(row.end), line_height)?.x;
            (right > left).then(|| {
                Bounds::from_corners(
                    gpui::point(left, top),
                    gpui::point(right, top + line_height),
                )
            })
        })
        .collect()
}

impl Render for TextField {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // The only place the blink starts: `caret_moved` drops the task, so the
        // next render brings it back in phase, solid beat first.
        if self.focus_handle.is_focused(_window) && caret_blink(cx) {
            if self.blink.is_none() {
                self.start_blink(cx);
            }
        } else {
            self.blink = None;
            self.caret_on = true;
        }
        let theme = Theme::of(cx);
        let mut key_context = gpui::KeyContext::default();
        key_context.add(KEY_CONTEXT);
        if self.shape.is_multiline() {
            key_context.add(MULTILINE_KEY_CONTEXT);
        }
        if let Some(extra) = self.key_context.clone() {
            key_context.add(extra);
        }
        div()
            .key_context(key_context)
            .track_focus(&self.focus_handle(cx))
            .cursor(CursorStyle::IBeam)
            .on_action(cx.listener(Self::backspace))
            .on_action(cx.listener(Self::delete))
            .on_action(cx.listener(Self::left))
            .on_action(cx.listener(Self::right))
            .on_action(cx.listener(Self::select_left))
            .on_action(cx.listener(Self::select_right))
            .on_action(cx.listener(Self::select_all))
            .on_action(cx.listener(Self::home))
            .on_action(cx.listener(Self::end))
            .on_action(cx.listener(Self::select_home))
            .on_action(cx.listener(Self::select_end))
            .on_action(cx.listener(Self::word_left))
            .on_action(cx.listener(Self::word_right))
            .on_action(cx.listener(Self::select_word_left))
            .on_action(cx.listener(Self::select_word_right))
            .on_action(cx.listener(Self::up))
            .on_action(cx.listener(Self::down))
            .on_action(cx.listener(Self::select_up))
            .on_action(cx.listener(Self::select_down))
            .on_action(cx.listener(Self::insert_newline))
            .on_action(cx.listener(Self::undo))
            .on_action(cx.listener(Self::redo))
            .on_action(cx.listener(Self::delete_word_left))
            .on_action(cx.listener(Self::delete_word_right))
            .on_action(cx.listener(Self::delete_to_line_start))
            .on_action(cx.listener(Self::delete_to_line_end))
            .on_action(cx.listener(Self::show_character_palette))
            .on_action(cx.listener(Self::paste))
            .on_action(cx.listener(Self::cut))
            .on_action(cx.listener(Self::copy))
            .on_mouse_down(MouseButton::Left, cx.listener(Self::on_mouse_down))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::on_mouse_up))
            .on_mouse_up_out(MouseButton::Left, cx.listener(Self::on_mouse_up))
            .on_scroll_wheel(cx.listener(Self::on_scroll_wheel))
            .w_full()
            .when(self.frame, |field| {
                field
                    .px(px(10.0))
                    .py(px(7.0))
                    .rounded(px(Theme::button_radius()))
                    .bg(theme.input_bg)
                    .border_1()
                    .border_color(if self.focus_handle.is_focused(_window) {
                        theme.ring
                    } else {
                        theme.border
                    })
            })
            .text_size(px(self.metrics.size()))
            .font_weight(self.metrics.weight)
            .line_height(px(self.metrics.line_height()))
            .text_color(theme.text)
            .child(TextFieldElement { field: cx.entity() })
    }
}

/// Paints the shaped lines plus selection and caret. A custom element because
/// all three are geometry derived from the shaped text, which only exists after
/// layout.
pub(super) struct TextFieldElement {
    field: Entity<TextField>,
}

pub(super) struct FieldPrepaint {
    lines: Vec<WrappedLine>,
    /// Top-left of the text, which is the box moved up by the scroll offset.
    origin: Point<Pixels>,
    cursor: Option<PaintQuad>,
    /// One quad per visual row each match covers.
    matches: Vec<PaintQuad>,
    /// One quad per visual row the selection covers.
    selection: Vec<PaintQuad>,
}

impl IntoElement for TextFieldElement {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for TextFieldElement {
    type RequestLayoutState = ();
    type PrepaintState = FieldPrepaint;

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&gpui::InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, ()) {
        let mut style = Style::default();
        style.size.width = relative(1.).into();
        let field = self.field.read(cx);
        // The field's own, here as everywhere: sizing the box off one height
        // and hit-testing it at another is how a click lands on the wrong row.
        let line_height = field.line_height();
        let shape = field.shape;

        let (min, max) = match shape {
            Shape::Line => {
                style.size.height = line_height.into();
                return (window.request_layout(style, [], cx), ());
            }
            Shape::Rows(rows) => {
                style.size.height = (line_height * rows.max(1) as f32).into();
                return (window.request_layout(style, [], cx), ());
            }
            // Growing needs the row count, and the row count needs shaping at
            // the width layout is still deciding — which is exactly what a
            // measured layout is for.
            Shape::Grow { min, max } => (min.max(1), max.max(min.max(1))),
        };

        let text = display_text(field).0;
        let id = window.request_measured_layout(style, move |known, available, window, _cx| {
            let text_style = window.text_style();
            let font_size = text_style.font_size.to_pixels(window.rem_size());
            // Prefer the width layout has already settled on. Taffy also probes
            // with min/max-content, where there is no width to wrap against —
            // and counting rows off unwrapped text under-reports them, which
            // would size the box for fewer lines than it goes on to paint.
            let wrap_width = known.width.or(match available.width {
                gpui::AvailableSpace::Definite(width) => Some(width),
                _ => None,
            });
            let run = TextRun {
                len: text.len(),
                font: text_style.font(),
                color: text_style.color,
                background_color: None,
                underline: None,
                strikethrough: None,
            };
            let count = window
                .text_system()
                .shape_text(text.clone(), font_size, &[run], wrap_width, None)
                .map(|lines| {
                    lines
                        .iter()
                        .map(|line| line.wrap_boundaries().len() + 1)
                        .sum::<usize>()
                })
                .unwrap_or(1);
            gpui::size(
                wrap_width.unwrap_or(px(0.)),
                line_height * count.clamp(min, max) as f32,
            )
        });
        (id, ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&gpui::InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> FieldPrepaint {
        let theme = Theme::of(cx).clone();
        let field = self.field.read(cx);
        let selected_range = field.selected_range.clone();
        let cursor = field.cursor_offset();
        let shape = field.shape;
        let marked_range = field.marked_range.clone();
        let matches: Vec<_> = field
            .matches
            .iter()
            .filter(|range| {
                range.start <= range.end
                    && field.content.is_char_boundary(range.start)
                    && field.content.is_char_boundary(range.end)
            })
            .cloned()
            .collect();
        let scrolled = field.scroll;
        let follow_caret = field.follow_caret;
        let style = window.text_style();

        let (text, is_placeholder) = display_text(field);
        let text_color = if is_placeholder {
            theme.text_faint
        } else {
            style.color
        };

        let run = TextRun {
            len: text.len(),
            font: style.font(),
            color: text_color,
            background_color: None,
            underline: None,
            strikethrough: None,
        };
        // Syntax first, then the IME composition range underlined over the top
        // of it: both cut the text into runs, and the underline has to land on
        // whatever colour is already there.
        let runs = if is_placeholder {
            vec![run]
        } else {
            coloured(&text, &field.spans, &run, &theme.syntax)
        };
        let runs = match marked_range.as_ref() {
            Some(marked) => underlined(runs, marked),
            None => runs,
        };

        let font_size = style.font_size.to_pixels(window.rem_size());
        let line_height = field.line_height();
        // A single line never wraps: it scrolls sideways instead, so shaping it
        // against the field's width would fold it into rows nothing can reach.
        let wrap_width = shape.is_multiline().then_some(bounds.size.width);
        let lines = window
            .text_system()
            .shape_text(text, font_size, &runs, wrap_width, None)
            .map(|lines| lines.into_vec())
            .unwrap_or_default();

        // Clamp every frame, not just when scrolling: the content this is
        // measured against shrinks under it — delete the last line while parked
        // at the bottom and an unclamped offset leaves the box showing nothing.
        //
        // Only one axis is ever live. Wrapped lines are shaped to the box width,
        // so `max.x` is zero for a multi-line field; a single line is one row
        // tall, so `max.y` is zero for a single-line one. Neither needs asking
        // which shape it is.
        let content_height: Pixels = lines.iter().map(|l| l.size(line_height).height).sum();
        let content_width = lines.iter().map(|l| l.width()).fold(px(0.), Pixels::max);
        let max = gpui::point(
            (content_width - bounds.size.width).max(px(0.)),
            (content_height - bounds.size.height).max(px(0.)),
        );
        let mut scroll = gpui::point(
            scrolled.x.clamp(px(0.), max.x),
            scrolled.y.clamp(px(0.), max.y),
        );
        if follow_caret && let Some(at) = position_for_offset(&lines, cursor, line_height) {
            if at.y < scroll.y {
                scroll.y = at.y;
            } else if at.y + line_height > scroll.y + bounds.size.height {
                scroll.y = at.y + line_height - bounds.size.height;
            }
            // The caret is the thing being kept in view, so it is its own width
            // that has to clear the right edge — not the character before it.
            if at.x < scroll.x {
                scroll.x = at.x;
            } else if at.x + CARET_WIDTH > scroll.x + bounds.size.width {
                scroll.x = at.x + CARET_WIDTH - bounds.size.width;
            }
            scroll.x = scroll.x.clamp(px(0.), max.x);
            scroll.y = scroll.y.clamp(px(0.), max.y);
        }
        self.field.update(cx, |field, _| {
            field.scroll = scroll;
            field.follow_caret = false;
        });
        let origin = bounds.origin - scroll;

        let matches = if is_placeholder {
            Vec::new()
        } else {
            matches
                .iter()
                .flat_map(|range| selection_rows(&lines, range, line_height))
                .map(|rect| {
                    fill(
                        Bounds::new(origin + rect.origin, rect.size),
                        theme.warning.opacity(0.25),
                    )
                })
                .collect()
        };

        let (selection, cursor) = if selected_range.is_empty() {
            let at = position_for_offset(&lines, cursor, line_height).unwrap_or_default();
            (
                Vec::new(),
                Some(fill(
                    // The font's size rather than the line's: leading is not
                    // the caret's to fill.
                    Bounds::new(
                        origin + at + gpui::point(px(0.), (line_height - font_size) / 2.),
                        gpui::size(CARET_WIDTH, font_size),
                    ),
                    theme.caret,
                )),
            )
        } else {
            (
                selection_rows(&lines, &selected_range, line_height)
                    .into_iter()
                    .map(|rect| {
                        fill(
                            Bounds::new(origin + rect.origin, rect.size),
                            theme.selection,
                        )
                    })
                    .collect(),
                None,
            )
        };

        FieldPrepaint {
            lines,
            origin,
            cursor,
            matches,
            selection,
        }
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&gpui::InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        let focus_handle = self.field.read(cx).focus_handle.clone();
        let caret_on = self.field.read(cx).caret_on;
        window.handle_input(
            &focus_handle,
            ElementInputHandler::new(bounds, self.field.clone()),
            cx,
        );
        let line_height = self.field.read(cx).line_height();
        // A drag that leaves the box is still a drag. `on_mouse_move` on the
        // field's own div fires only while the box is the thing under the
        // pointer, so a run dragged past the edge froze at the last character
        // inside it — and the last line of a full box was unreachable, since
        // reaching it means passing the edge. This is the window's own move,
        // which arrives wherever the pointer went.
        //
        // Registered every frame because that is the contract: the listener is
        // cleared with the frame that installed it.
        let dragged = self.field.clone();
        window.on_mouse_event(move |event: &MouseMoveEvent, phase, _window, cx| {
            // The button being down is what makes this a drag. `is_selecting`
            // is only cleared on the release, so a press whose release went
            // somewhere we never heard about would otherwise leave a plain
            // hover dragging the run around.
            if phase != DispatchPhase::Bubble || !event.dragging() {
                return;
            }
            dragged.update(cx, |field, cx| {
                field.drag_to(event.position, line_height, cx);
            });
        });
        let lines = std::mem::take(&mut prepaint.lines);
        let matches = std::mem::take(&mut prepaint.matches);
        let selection = std::mem::take(&mut prepaint.selection);
        let cursor = prepaint.cursor.take();
        let origin = prepaint.origin;

        // Scrolled text runs past the box in both directions, so everything the
        // field draws is masked to it — text, selection and caret alike.
        window.with_content_mask(Some(gpui::ContentMask::new(bounds)), |window| {
            for quad in matches.into_iter().chain(selection) {
                window.paint_quad(quad);
            }

            let mut top = origin;
            for line in &lines {
                line.paint(top, line_height, gpui::TextAlign::Left, None, window, cx)
                    .ok();
                top.y += line.size(line_height).height;
            }

            // The caret only exists while focused — an unfocused field showing
            // one reads as two cursors on screen.
            if focus_handle.is_focused(window)
                && caret_on
                && let Some(cursor) = cursor
            {
                window.paint_quad(cursor);
            }
        });

        self.field.update(cx, |field, _| {
            field.last_layout = lines;
            field.last_bounds = Some(bounds);
        });
    }
}
