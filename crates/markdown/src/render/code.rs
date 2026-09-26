//! Code blocks, source mode, and the copy button.

use super::*;

/// Paint a document's own markdown source: a fence's caret, selection and hit
/// testing, without a fence's box, band or copy button.
///
/// The caret is a [`Cursor`] at block 0 in [`Part::Code`] — what a document
/// held as one fence answers to, which is how an editor holds its source.
/// Wrapping is not optional here: a paragraph is one line of markdown, and a
/// source view that scrolled sideways would hide most of it.
pub fn render_source(code: &str, editing: Editing, cx: &mut App) -> AnyElement {
    let Editing {
        selection,
        caret_on,
        layouts,
        annotations,
        typography,
        ..
    } = editing;
    // The same reset `render_with` opens with, and for the same reason: the
    // positions this frame records are the ones the next click resolves
    // against, and last frame's have to go first.
    let reset = layouts.map(|layouts| {
        let layouts = layouts.clone();
        canvas(move |_, _, _| layouts.clear(), |_, _, _, _| ())
            .absolute()
            .size(px(0.0))
    });
    let theme = Theme::of(cx).clone();
    let typography = typography.unwrap_or_else(|| Typography::of(cx));
    let overlay = Overlay {
        block: 0,
        part: Part::Code,
        selection,
        caret_on,
        layouts,
        annotations,
        placeholder: None,
        caption: Caption::default(),
        // The source view is one fence and holds no task block.
        toggle: None,
        // It paints no band, so there is nowhere for the button to float.
        copy: CopyButton::Hidden,
        base: None,
        highlight: crate::marks::highlight_paint_of(cx),
    };
    let (underlay, lines) = code_lines(
        Some(crate::source::LANGUAGES[0]),
        code,
        overlay,
        &typography,
        &theme,
        cx,
    );
    // Keep each number beside its source line, including wrapped and empty lines.
    let style = crate::SourceStyle::of(cx);
    let digits = lines.len().to_string().len().max(style.gutter_min_digits);
    let gap = style.gutter_gap.max(0.0) * typography.code.size();
    let gutter_width = digits as f32 * typography.code.size() + gap;
    let lines = lines
        .into_iter()
        .enumerate()
        .map(|(index, line)| {
            if !style.line_numbers {
                return line;
            }
            div()
                .flex()
                .items_start()
                .child(
                    div()
                        .w(px(gutter_width))
                        .flex_shrink_0()
                        .pr(px(gap))
                        .font_family(theme.font_mono.clone())
                        .text_color(style.gutter_color.unwrap_or(theme.text_faint))
                        .text_right()
                        .child((index + 1).to_string()),
                )
                .child(div().flex_1().min_w_0().child(line))
                .into_any_element()
        })
        .collect();
    div()
        .flex()
        .flex_col()
        .children(reset)
        .child(code_body(0, underlay, lines, &typography, true))
        .into_any_element()
}

/// The shaped lines of a fence, and the canvas that paints the caret, the
/// selection and the annotations over them.
pub(super) fn code_lines(
    language: Option<&str>,
    code: &str,
    overlay: Overlay,
    typography: &Typography,
    theme: &Theme,
    cx: &App,
) -> (AnyElement, Vec<AnyElement>) {
    let ix = overlay.block;
    // Highlighting recolors runs only — layout does not move, so a build with
    // no highlighter installed paints the same block in one plain run.
    // Markdown is the one language this crate can colour on its own, which is
    // what a source view is painted with where no highlighter reaches.
    let spans = crate::highlight::spans(cx, language, code).or_else(|| {
        language
            .filter(|language| crate::source::is_markdown(language))
            .map(|_| crate::source::spans(code))
    });
    let mono = font(theme.font_mono.clone());
    let run = |len: usize, color: Hsla| TextRun {
        len,
        font: mono.clone(),
        color,
        background_color: None,
        underline: None,
        strikethrough: None,
    };
    // Each source line's own layout, with the slice of the code it covers —
    // the caret and a click both resolve through these. A wrapped line is
    // several rows of one layout, which is the case `range_rects` already
    // walks for a paragraph.
    let mut rows: Vec<(Range<usize>, TextLayout)> = Vec::new();
    let mut offset = 0usize;
    let lines: Vec<AnyElement> = code
        .split('\n')
        .map(|line| {
            let start = offset;
            offset += line.len() + 1;
            let mut runs = Vec::new();
            // Runs are measured within the line; spans are byte ranges over the
            // whole block, so every span is clipped to the line and rebased.
            let mut pos = 0usize;
            if let Some(spans) = &spans {
                let end = start + line.len();
                for (range, kind) in spans.iter().filter(|(r, _)| r.end > start && r.start < end) {
                    let s = range.start.clamp(start, end) - start;
                    let e = range.end.min(end) - start;
                    if s > pos {
                        runs.push(run(s - pos, theme.text));
                    }
                    runs.push(run(e - s, theme.syntax.color(*kind)));
                    pos = e;
                }
            }
            if pos < line.len() {
                runs.push(run(line.len() - pos, theme.text));
            }
            if runs.is_empty() {
                runs.push(run(0, theme.text));
            }
            let styled = StyledText::new(SharedString::from(line.to_string())).with_runs(runs);
            rows.push((start..start + line.len(), styled.layout().clone()));
            styled.into_any_element()
        })
        .collect();

    let caret = overlay.caret_painted();
    let selected = overlay.selected(code.len());
    let sink = overlay.layouts.cloned();
    let code_size = typography.code.size();
    let annotated = overlay.annotated(code.len(), theme);
    let (caret_color, selection_color) = (theme.caret, theme.selection);
    let underlay = canvas(
        |_, _, _| (),
        move |_, _, window, _| {
            for (span, layout) in &rows {
                if let Some(sink) = &sink {
                    sink.record(ix, Part::Code, span.clone(), layout.clone());
                }
                for (range, wash) in &annotated {
                    let (from, to) = (range.start.max(span.start), range.end.min(span.end));
                    if from < to {
                        for rect in
                            range_rects(layout, &(from - span.start..to - span.start), 0.0, 0.0)
                        {
                            window.paint_quad(quad(
                                rect,
                                px(2.0),
                                *wash,
                                px(0.0),
                                gpui::transparent_black(),
                                BorderStyle::default(),
                            ));
                        }
                    }
                }
                if let Some(range) = &selected {
                    let (from, to) = (range.start.max(span.start), range.end.min(span.end));
                    if from < to {
                        for rect in
                            range_rects(layout, &(from - span.start..to - span.start), 0.0, 0.0)
                        {
                            window.paint_quad(quad(
                                rect,
                                px(2.0),
                                selection_color,
                                px(0.0),
                                gpui::transparent_black(),
                                BorderStyle::default(),
                            ));
                        }
                    }
                }
                if let Some(offset) = caret.filter(|at| span.contains(at) || *at == span.end)
                    && let Some(head) = layout.position_for_index(offset - span.start)
                {
                    window.paint_quad(quad(
                        caret_quad(head, code_size, layout.line_height()),
                        px(0.0),
                        caret_color,
                        px(0.0),
                        gpui::transparent_black(),
                        BorderStyle::default(),
                    ));
                }
            }
        },
    )
    .absolute()
    .size_full();

    (underlay.into_any_element(), lines)
}

pub(super) fn code_block(
    language: Option<&str>,
    code: &str,
    overlay: Overlay,
    typography: &Typography,
    theme: &Theme,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    let ix = overlay.block;
    let (underlay, lines) = code_lines(language, code, overlay, typography, theme, cx);
    let body = code_body(ix, underlay, lines, typography, Layout::of(cx).wrap_code);

    div()
        .rounded(px(Theme::panel_radius()))
        .bg(theme.ink(0.035))
        .border_1()
        .border_color(theme.border)
        .overflow_hidden()
        .relative()
        // The band is unconditional: it is where the copy button already floats,
        // and where a host puts its language control — which needs somewhere to
        // sit on a block that has no language yet.
        .child(
            div()
                .relative()
                .flex()
                .flex_row()
                .items_center()
                .px(px(CODE_PADDING_X))
                .py(px(5.0))
                .border_b_1()
                .border_color(theme.border)
                .bg(theme.ink(0.02))
                .text_style(TextStyle::Subheadline)
                .text_color(match language {
                    Some(_) => theme.text_muted,
                    None => theme.text_faint,
                })
                // The label's own box, not the band's: a host hanging a picker
                // here wants it around the word, and only the word knows how
                // wide the word is.
                .child(
                    div()
                        .relative()
                        .children(overlay.layouts.map(|layouts| {
                            let layouts = layouts.clone();
                            canvas(
                                move |bounds, _, _| layouts.record_language(ix, bounds),
                                |_, _, _, _| (),
                            )
                            .absolute()
                            .size_full()
                        }))
                        .child(SharedString::from(
                            language.unwrap_or(PLAIN_LANGUAGE).to_string(),
                        )),
                ),
        )
        .child(body)
        .children(
            (overlay.copy == CopyButton::Shown).then(|| copy_button(code, ix, theme, window, cx)),
        )
        .into_any_element()
}

/// The lines of a fence, wrapped to the block or scrolling sideways under it.
pub(super) fn code_body(
    ix: usize,
    underlay: AnyElement,
    lines: Vec<AnyElement>,
    typography: &Typography,
    wrap: bool,
) -> AnyElement {
    let column = div()
        .flex()
        .flex_col()
        .px(px(CODE_PADDING_X))
        .children(lines);
    let body = div()
        .id(ElementId::named_usize("md-code", ix))
        .relative()
        .py(px(CODE_PADDING_Y))
        .text_size(px(typography.code.size()))
        .line_height(px(typography.code.line_height()))
        .child(underlay);
    if wrap {
        // The column is the block's width here rather than its widest line's,
        // which is what gives the text something to wrap against.
        body.child(column.w_full()).into_any_element()
    } else {
        ui::scroll::Viewport::new(
            format!("md-code-scroll-{ix}"),
            body.flex()
                .flex_row()
                .whitespace_nowrap()
                // The padding belongs to the lines, not to the scroller: a scroll
                // container's trailing padding is not part of what it will scroll
                // to, so the last characters of a long line sit behind the right
                // edge with nowhere left to go. As a row's only item this column is
                // sized by its widest line, and the padding rides along inside that
                // width.
                .child(column.items_start()),
            gpui::Axis::Horizontal,
        )
        .into_any_element()
    }
}

/// A copy button that owns its own feedback.
///
/// The state is the element's, not the caller's: a component library cannot ask
/// every host to thread a handler and a "which block is showing Copied" index
/// through its render tree just to put a button on a code block. It resets when
/// the pointer leaves, which needs no clock.
pub(super) fn copy_button(
    code: &str,
    ix: usize,
    theme: &Theme,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    let copied = window.use_keyed_state(ElementId::named_usize("md-copied", ix), cx, |_, _| false);
    let showing = *copied.read(cx);
    let text: SharedString = code.to_string().into();

    div()
        .id(ElementId::named_usize("md-copy", ix))
        .absolute()
        .top(px(3.0))
        .right(px(5.0))
        .h(px(20.0))
        .px(px(6.0))
        .rounded(px(5.0))
        .flex()
        .items_center()
        .cursor_pointer()
        .text_style(TextStyle::Caption)
        .text_color(theme.text_muted)
        .hover(|el| el.bg(theme.element_hover))
        .child(if showing { "Copied" } else { "Copy" })
        .on_click({
            let copied = copied.clone();
            move |_, _, cx| {
                cx.write_to_clipboard(gpui::ClipboardItem::new_string(text.to_string()));
                copied.update(cx, |state, cx| {
                    *state = true;
                    cx.notify();
                });
            }
        })
        .on_hover(move |hovering, _, cx| {
            if !*hovering && *copied.read(cx) {
                copied.update(cx, |state, cx| {
                    *state = false;
                    cx.notify();
                });
            }
        })
        .into_any_element()
}
