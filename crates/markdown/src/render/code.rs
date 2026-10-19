//! Code blocks.

use super::*;
use ui::input::overdraw::Layers;

/// The shaped lines of a fence, the canvas that paints the selection and the
/// annotations under them, and the one that paints the caret over them.
pub(super) fn code_lines(
    language: Option<&str>,
    code: &str,
    overlay: Overlay,
    typography: &Typography,
    theme: &Theme,
    cx: &App,
) -> (AnyElement, AnyElement, Vec<AnyElement>) {
    let ix = overlay.block;
    // Highlighting recolors runs only — layout does not move, so a build with
    // no highlighter installed paints the same block in one plain run.
    // Markdown is the one language this crate can colour on its own, which is
    // what a source view is painted with where no highlighter reaches.
    let spans = crate::fence::highlight::spans(cx, language, code).or_else(|| {
        language
            .filter(|language| crate::source::is_markdown(language))
            .map(|_| crate::source::spans(code))
    });
    // Each source line's own layout, with the slice of the code it covers —
    // the caret and a click both resolve through these. A wrapped line is
    // several rows of one layout, which is the case `range_rects` already
    // walks for a paragraph.
    let paint = RowPaint::new(&overlay, code, typography.code.size(), theme);
    let mut rows: Vec<(Range<usize>, TextLayout, Vec<TextRun>)> = Vec::new();
    let mut offset = 0usize;
    let lines: Vec<AnyElement> = code
        .split('\n')
        .map(|line| {
            let start = offset;
            offset += line.len() + 1;
            let (styled, runs) = code_line(line, start, spans.as_deref(), theme);
            rows.push((start..start + line.len(), styled.layout().clone(), runs));
            styled.into_any_element()
        })
        .collect();

    let sink = overlay.layouts.cloned();
    let over = {
        let (rows, paint) = (rows.clone(), paint.clone());
        canvas(
            |_, _, _| (),
            move |_, _, window, _| {
                for (span, layout, runs) in &rows {
                    paint.paint_over(span, layout, runs, window);
                }
            },
        )
        .absolute()
        .size_full()
    };
    let underlay = canvas(
        |_, _, _| (),
        move |_, _, window, _| {
            for (span, layout, _) in &rows {
                if let Some(sink) = &sink {
                    sink.record(
                        ix,
                        Part::Code,
                        span.clone(),
                        layout.clone(),
                        Shown::default(),
                    );
                }
                paint.paint_under(span, layout, window);
            }
        },
    )
    .absolute()
    .size_full();

    (underlay.into_any_element(), over.into_any_element(), lines)
}

/// A highlighted byte range of a text.
pub(super) type Span = (Range<usize>, theme::HighlightKind);

/// One line of code, coloured by the spans over the text it came from, and
/// its runs.
///
/// Runs are measured within the line; spans are byte ranges over the whole
/// text, so every span is clipped to the line and rebased.
pub(super) fn code_line(
    line: &str,
    start: usize,
    spans: Option<&[Span]>,
    theme: &Theme,
) -> (StyledText, Vec<TextRun>) {
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
    let styled = StyledText::new(SharedString::from(line.to_string())).with_runs(runs.clone());
    (styled, runs)
}

/// What paints with a line of code: the selection and the annotations under
/// it, the caret and the text they leave unreadable over it, each clipped to the slice of the text the line
/// covers.
#[derive(Clone)]
pub(super) struct RowPaint {
    caret: Option<usize>,
    /// The grapheme after the caret, over the whole text.
    glyph: Option<Range<usize>>,
    affinity: Affinity,
    shape: ui::input::CaretShape,
    height: ui::input::CaretHeight,
    hollow: bool,
    font: SharedString,
    /// What is painted under the code, over the whole text.
    layers: Layers<()>,
    caret_color: Hsla,
    /// What the code is painted on.
    page: Hsla,
    code_size: f32,
}

impl RowPaint {
    /// The same paint, its caret hollow or not.
    pub(super) fn hollow(&self, hollow: bool) -> Self {
        Self {
            hollow,
            ..self.clone()
        }
    }

    pub(super) fn new(overlay: &Overlay, code: &str, code_size: f32, theme: &Theme) -> Self {
        let caret = overlay.caret_painted();
        Self {
            caret,
            glyph: caret.and_then(|offset| overlay.covered(code, offset)),
            affinity: overlay.affinity(),
            shape: overlay.caret_shape,
            height: overlay.caret_height,
            hollow: overlay.caret_hollow(),
            font: theme.font_mono.clone(),
            layers: {
                let mut layers = Layers::new();
                for (range, color) in overlay.annotated(code.len(), theme) {
                    layers = layers.wash(range, color, ());
                }
                if let Some(range) = overlay.selected(code.len()) {
                    layers = layers.select(range, theme.selection, ());
                }
                layers
            },
            caret_color: theme.caret,
            page: theme.bg,
            code_size,
        }
    }

    /// The grapheme a solid block covers.
    fn cut(&self) -> Option<&Range<usize>> {
        self.glyph
            .as_ref()
            .filter(|_| self.shape.cuts_out(self.hollow))
    }

    pub(super) fn paint_under(
        &self,
        span: &Range<usize>,
        layout: &TextLayout,
        window: &mut Window,
    ) {
        for (range, color, ()) in self.layers.clipped(span).painted() {
            for rect in range_rects(layout, range, 0.0, 0.0) {
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
    }

    /// `runs` are the line's, `span` its slice of the whole text.
    pub(super) fn paint_over(
        &self,
        span: &Range<usize>,
        layout: &TextLayout,
        runs: &[TextRun],
        window: &mut Window,
    ) {
        let layers = self.layers.clipped(span);
        ui::input::overdraw::paint(
            layout.line_layouts().iter().map(|line| &**line),
            layout.bounds().origin,
            layout.line_height(),
            px(2.0),
            &ui::input::overdraw::overdraws(runs, self.page, &layers),
            window,
        );
        if let Some(offset) = self.caret.filter(|at| span.contains(at) || *at == span.end) {
            let glyph = self
                .cut()
                .map(|glyph| glyph.start - span.start..glyph.end - span.start);
            paint_caret(
                layout,
                offset - span.start,
                self.affinity,
                glyph.as_ref(),
                CaretPaint {
                    shape: self.shape,
                    height: self.height,
                    hollow: self.hollow,
                    color: self.caret_color,
                    cover: ui::input::caret::cover(
                        runs,
                        offset - span.start,
                        self.page,
                        &layers,
                        self.caret_color,
                    ),
                    size: self.code_size,
                    face: font(self.font.clone()),
                },
                window,
            );
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn code_block(
    language: Option<&str>,
    code: &str,
    note: Option<SharedString>,
    overlay: Overlay,
    typography: &Typography,
    theme: &Theme,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    let ix = overlay.block;
    let (underlay, over, lines) = code_lines(language, code, overlay, typography, theme, cx);
    let body = code_body(
        ix,
        underlay,
        over,
        lines,
        typography,
        cx.markdown_layout().wrap_code,
    );

    fence_panel(theme)
        // The band is unconditional: it is where the copy button already floats,
        // and where a host puts its language control — which needs somewhere to
        // sit on a block that has no language yet.
        .child(
            fence_band(theme)
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
        .children(note.map(|note| fence_note(note, theme)))
        .children(
            (overlay.copy == CopyButton::Shown).then(|| copy_button(code, ix, theme, window, cx)),
        )
        .into_any_element()
}

/// The lines of a fence, wrapped to the block or scrolling sideways under it.
pub(super) fn code_body(
    ix: usize,
    underlay: AnyElement,
    over: AnyElement,
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
        body.child(column.w_full()).child(over).into_any_element()
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
                .child(column.items_start())
                .child(over),
            gpui::Axis::Horizontal,
        )
        .into_any_element()
    }
}
