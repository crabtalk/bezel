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
        keep,
        scroll,
        ..
    } = editing;
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
    let style = crate::SourceStyle::of(cx);
    let count = code.split('\n').count();
    let gutter = Gutter::new(&style, count, &typography, &theme);

    let Some(layouts) = layouts else {
        let (underlay, lines) = code_lines(
            Some(crate::source::LANGUAGES[0]),
            code,
            overlay,
            &typography,
            &theme,
            cx,
        );
        let lines = lines
            .into_iter()
            .enumerate()
            .map(|(index, line)| gutter.row(index, line))
            .collect();
        return div()
            .flex()
            .flex_col()
            .child(code_body(0, underlay, lines, &typography, true))
            .into_any_element();
    };

    // Built a line at a time by the same column the blocks are: a line is to
    // this text what a block is to a document.
    let language = crate::source::LANGUAGES[0];
    let spans: Option<Rc<[Span]>> = crate::highlight::spans(cx, Some(language), code)
        .or_else(|| crate::source::is_markdown(language).then(|| crate::source::spans(code)))
        .map(Into::into);
    let mut ranges = Vec::with_capacity(count);
    let mut offset = 0usize;
    for line in code.split('\n') {
        ranges.push(offset..offset + line.len());
        offset += line.len() + 1;
    }
    let line_of = |at: usize| {
        ranges
            .partition_point(|range| range.end < at)
            .min(count - 1)
    };
    let mut kept: Vec<usize> = keep.to_vec();
    if let Some(selection) = selection {
        kept.push(line_of(selection.head.offset));
        kept.push(line_of(selection.anchor.offset));
    }
    let line = px(typography.code.line_height());
    let keys: Vec<u64> = ranges
        .iter()
        .map(|range| {
            let mut hasher = DefaultHasher::new();
            "source".hash(&mut hasher);
            code[range.clone()].hash(&mut hasher);
            typography.code.size().to_bits().hash(&mut hasher);
            hasher.finish()
        })
        .collect();
    layouts.prune(&keys);
    let indent = px(gutter.width() + 2.0 * CODE_PADDING_X);
    let guesses: Vec<Guess> = ranges
        .iter()
        .map(|range| Guess {
            chars: range.len(),
            line,
            rows: 0,
            extra: px(0.0),
            indent,
        })
        .collect();
    let paint = RowPaint {
        caret: overlay.caret_painted(),
        selected: overlay.selected(code.len()),
        annotated: overlay.annotated(code.len(), &theme),
        caret_color: theme.caret,
        selection_color: theme.selection,
        code_size: typography.code.size(),
    };
    let code: Rc<str> = code.into();
    let sink = layouts.clone();
    let ranges: Rc<[Range<usize>]> = ranges.into();
    let column = Column {
        layouts: layouts.clone(),
        keys: keys.into(),
        gaps: vec![px(0.0); count].into(),
        guesses: guesses.into(),
        keep: kept,
        scroll: scroll.cloned(),
        build: Box::new(move |index, _, _| {
            let span = ranges[index].clone();
            let styled = code_line(&code[span.clone()], span.start, spans.as_deref(), &theme);
            let layout = styled.layout().clone();
            let (sink, paint) = (sink.clone(), paint.clone());
            let underlay = canvas(
                |_, _, _| (),
                move |_, _, window, _| {
                    sink.record(0, Part::Code, span.clone(), layout.clone());
                    paint.paint(&span, &layout, window);
                },
            )
            .absolute()
            .size_full();
            let text = div().relative().child(underlay).child(styled);
            div()
                .px(px(CODE_PADDING_X))
                .child(gutter.row(index, text.into_any_element()))
                .into_any_element()
        }),
    };
    div()
        .flex()
        .flex_col()
        .py(px(CODE_PADDING_Y))
        .text_size(px(typography.code.size()))
        .line_height(line)
        .child(column)
        .into_any_element()
}

/// The line numbers beside a source view, when it shows them.
#[derive(Clone)]
struct Gutter {
    shown: bool,
    width: f32,
    gap: f32,
    color: Hsla,
    font: SharedString,
}

impl Gutter {
    fn new(
        style: &crate::SourceStyle,
        lines: usize,
        typography: &Typography,
        theme: &Theme,
    ) -> Self {
        let digits = lines.to_string().len().max(style.gutter_min_digits);
        let gap = style.gutter_gap.max(0.0) * typography.code.size();
        Self {
            shown: style.line_numbers,
            width: digits as f32 * typography.code.size() + gap,
            gap,
            color: style.gutter_color.unwrap_or(theme.text_faint),
            font: theme.font_mono.clone(),
        }
    }

    fn width(&self) -> f32 {
        if self.shown { self.width } else { 0.0 }
    }

    /// Keeps each number beside its source line, including wrapped and empty
    /// lines.
    fn row(&self, index: usize, line: AnyElement) -> AnyElement {
        if !self.shown {
            return line;
        }
        div()
            .flex()
            .items_start()
            .child(
                div()
                    .w(px(self.width))
                    .flex_shrink_0()
                    .pr(px(self.gap))
                    .font_family(self.font.clone())
                    .text_color(self.color)
                    .text_right()
                    .child((index + 1).to_string()),
            )
            .child(div().flex_1().min_w_0().child(line))
            .into_any_element()
    }
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
            let styled = code_line(line, start, spans.as_deref(), theme);
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
    let paint = RowPaint {
        caret,
        selected,
        annotated,
        caret_color,
        selection_color,
        code_size,
    };
    let underlay = canvas(
        |_, _, _| (),
        move |_, _, window, _| {
            for (span, layout) in &rows {
                if let Some(sink) = &sink {
                    sink.record(ix, Part::Code, span.clone(), layout.clone());
                }
                paint.paint(span, layout, window);
            }
        },
    )
    .absolute()
    .size_full();

    (underlay.into_any_element(), lines)
}

/// A highlighted byte range of a text.
type Span = (Range<usize>, theme::HighlightKind);

/// One line of code, coloured by the spans over the text it came from.
///
/// Runs are measured within the line; spans are byte ranges over the whole
/// text, so every span is clipped to the line and rebased.
fn code_line(line: &str, start: usize, spans: Option<&[Span]>, theme: &Theme) -> StyledText {
    let mono = font(theme.font_mono.clone());
    let run = |len: usize, color: Hsla| TextRun {
        len,
        font: mono.clone(),
        color,
        background_color: None,
        underline: None,
        strikethrough: None,
    };
    let mut runs = Vec::new();
    let mut pos = 0usize;
    if let Some(spans) = spans {
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
    StyledText::new(SharedString::from(line.to_string())).with_runs(runs)
}

/// What paints under a line of code: the annotations, the selection and the
/// caret, each clipped to the slice of the text the line covers.
#[derive(Clone)]
struct RowPaint {
    caret: Option<usize>,
    selected: Option<Range<usize>>,
    annotated: Vec<(Range<usize>, Hsla)>,
    caret_color: Hsla,
    selection_color: Hsla,
    code_size: f32,
}

impl RowPaint {
    fn paint(&self, span: &Range<usize>, layout: &TextLayout, window: &mut Window) {
        let wash = |range: &Range<usize>, color: Hsla, window: &mut Window| {
            let (from, to) = (range.start.max(span.start), range.end.min(span.end));
            if from < to {
                for rect in range_rects(layout, &(from - span.start..to - span.start), 0.0, 0.0) {
                    window.paint_quad(quad(
                        rect,
                        px(2.0),
                        color,
                        px(0.0),
                        gpui::transparent_black(),
                        BorderStyle::default(),
                    ));
                }
            }
        };
        for (range, color) in &self.annotated {
            wash(range, *color, window);
        }
        if let Some(range) = &self.selected {
            wash(range, self.selection_color, window);
        }
        if let Some(offset) = self.caret.filter(|at| span.contains(at) || *at == span.end)
            && let Some(head) = layout.position_for_index(offset - span.start)
        {
            window.paint_quad(quad(
                caret_quad(head, self.code_size, layout.line_height()),
                px(0.0),
                self.caret_color,
                px(0.0),
                gpui::transparent_black(),
                BorderStyle::default(),
            ));
        }
    }
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
