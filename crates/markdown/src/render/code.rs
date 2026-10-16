//! Code blocks, source mode, and the copy button.

use super::*;
use ui::input::overdraw::Layers;

/// A source text and what [`render_source`] reads off it every frame: its
/// lines, a hash of each, and its highlight spans.
///
/// [`SourceText::new`] walks and highlights the whole text.
/// [`SourceText::edit`] redoes only the lines an edit touched and shifts the
/// spans around it; the [`SpanJob`] it answers highlights the new text in full,
/// on any thread, for [`SourceText::set_spans`]. [`SourceText::rehighlight`]
/// answers one when the installed highlighter has changed.
pub struct SourceText {
    code: Arc<str>,
    ranges: Rc<[Range<usize>]>,
    hashes: Rc<[u64]>,
    spans: Option<Rc<[Span]>>,
    /// Moved by every edit and rehighlight. A [`SpanJob`] carries the one it
    /// was taken at.
    generation: u64,
    /// The highlighter generation the last [`SpanJob`], or the build, used.
    highlighter: u64,
    sized: RefCell<Option<Sized>>,
}

/// The per-line tables that also depend on what the text is set at.
#[derive(Clone)]
struct Sized {
    /// The text size, line height and indent these were built for.
    at: (u32, Pixels, Pixels),
    keys: Rc<[u64]>,
    gaps: Rc<[Pixels]>,
    guesses: Rc<[Guess]>,
}

/// Highlighting a [`SourceText`] in full. `Send`, so it can run off the thread
/// that paints.
pub struct SpanJob {
    code: Arc<str>,
    highlighter: Option<crate::Highlighter>,
    generation: u64,
}

/// What a [`SpanJob`] found, for [`SourceText::set_spans`].
pub struct Spans {
    spans: Option<Vec<Span>>,
    generation: u64,
}

impl SpanJob {
    pub fn run(self) -> Spans {
        Spans {
            spans: highlight_source(self.highlighter, &self.code),
            generation: self.generation,
        }
    }
}

fn highlight_source(highlighter: Option<crate::Highlighter>, code: &str) -> Option<Vec<Span>> {
    let language = crate::source::LANGUAGES[0];
    highlighter
        .and_then(|highlighter| highlighter(language, code))
        .or_else(|| crate::source::is_markdown(language).then(|| crate::source::spans(code)))
}

impl SourceText {
    pub fn new(code: impl Into<Arc<str>>, cx: &App) -> Self {
        let code: Arc<str> = code.into();
        let spans = highlight_source(crate::highlight::installed(cx), &code).map(Into::into);
        let ranges = split_lines(&code, 0..code.len());
        let hashes = ranges.iter().map(|range| hash_line(&code, range)).collect();
        Self {
            code,
            ranges: ranges.into(),
            hashes,
            spans,
            generation: 0,
            highlighter: crate::highlight::generation(cx),
            sized: RefCell::new(None),
        }
    }

    pub fn text(&self) -> &str {
        &self.code
    }

    /// Lines, counting the empty one after a trailing newline.
    pub fn lines(&self) -> usize {
        self.ranges.len()
    }

    /// The line holding byte `offset`; past the end, the last line.
    pub fn line_of(&self, offset: usize) -> usize {
        self.ranges
            .partition_point(|range| range.end < offset)
            .min(self.ranges.len() - 1)
    }

    /// The highlight spans, in bytes and in order. Between an [`Self::edit`]
    /// and the [`Self::set_spans`] that follows it, the spans from before the
    /// edit, shifted past it, and none over the bytes it replaced.
    pub fn spans(&self) -> Option<&[(Range<usize>, theme::HighlightKind)]> {
        self.spans.as_deref()
    }

    /// Make the text `code`. The change is taken to be the one run of bytes
    /// between the longest prefix and suffix the two texts share. `None` when
    /// the text is unchanged.
    pub fn edit(&mut self, code: &str, cx: &App) -> Option<SpanJob> {
        let old = self.code.as_bytes();
        let new = code.as_bytes();
        let prefix = old.iter().zip(new).take_while(|(a, b)| a == b).count();
        if prefix == old.len() && prefix == new.len() {
            return None;
        }
        let room = old.len().min(new.len()) - prefix;
        let suffix = old
            .iter()
            .rev()
            .zip(new.iter().rev())
            .take(room)
            .take_while(|(a, b)| a == b)
            .count();
        let old_end = old.len() - suffix;
        let delta = (new.len() - suffix) as isize - old_end as isize;
        let shift = |at: usize| at.wrapping_add_signed(delta);

        let first = self.line_of(prefix);
        let last = self.line_of(old_end);
        let code: Arc<str> = code.into();
        let lines = split_lines(
            &code,
            self.ranges[first].start..shift(self.ranges[last].end),
        );
        let hashes: Vec<u64> = lines.iter().map(|range| hash_line(&code, range)).collect();
        let cut = first..last + 1;

        if let Some(sized) = self.sized.get_mut() {
            let (size, line, indent) = sized.at;
            sized.keys = splice(
                &sized.keys,
                cut.clone(),
                hashes.iter().map(|hash| key(*hash, size)),
            );
            sized.guesses = splice(
                &sized.guesses,
                cut.clone(),
                lines.iter().map(|range| guess(range, line, indent)),
            );
            sized.gaps = vec![px(0.0); sized.keys.len()].into();
        }
        self.hashes = splice(&self.hashes, cut.clone(), hashes);
        self.ranges = self.ranges[..first]
            .iter()
            .cloned()
            .chain(lines)
            .chain(
                self.ranges[last + 1..]
                    .iter()
                    .map(|range| shift(range.start)..shift(range.end)),
            )
            .collect();
        self.spans = self.spans.as_ref().map(|spans| {
            let mut kept = Vec::with_capacity(spans.len());
            for (range, kind) in spans.iter() {
                if range.start < prefix {
                    kept.push((range.start..range.end.min(prefix), *kind));
                }
                if range.end > old_end {
                    kept.push((shift(range.start.max(old_end))..shift(range.end), *kind));
                }
            }
            kept.into()
        });
        self.code = code;
        Some(self.span_job(cx))
    }

    /// Highlight the text again if the installed highlighter has changed since
    /// it was last highlighted. The spans stand until the job's land.
    pub fn rehighlight(&mut self, cx: &App) -> Option<SpanJob> {
        (self.highlighter != crate::highlight::generation(cx)).then(|| self.span_job(cx))
    }

    fn span_job(&mut self, cx: &App) -> SpanJob {
        self.generation += 1;
        self.highlighter = crate::highlight::generation(cx);
        SpanJob {
            code: self.code.clone(),
            highlighter: crate::highlight::installed(cx),
            generation: self.generation,
        }
    }

    /// Take what a [`SpanJob`] found. `false`, and nothing taken, when the text
    /// has been edited or rehighlighted since the job was made.
    pub fn set_spans(&mut self, spans: Spans) -> bool {
        if spans.generation != self.generation {
            return false;
        }
        self.spans = spans.spans.map(Into::into);
        true
    }

    fn sized(&self, size: f32, line: Pixels, indent: Pixels) -> Sized {
        let at = (size.to_bits(), line, indent);
        let mut held = self.sized.borrow_mut();
        if let Some(sized) = held.as_ref()
            && sized.at == at
        {
            return sized.clone();
        }
        let sized = Sized {
            at,
            keys: self.hashes.iter().map(|hash| key(*hash, at.0)).collect(),
            gaps: vec![px(0.0); self.ranges.len()].into(),
            guesses: self
                .ranges
                .iter()
                .map(|range| guess(range, line, indent))
                .collect(),
        };
        *held = Some(sized.clone());
        sized
    }
}

/// The lines of `code` within `within`, which starts at a line's start and
/// ends at a line's end.
fn split_lines(code: &str, within: Range<usize>) -> Vec<Range<usize>> {
    let mut ranges = Vec::new();
    let mut at = within.start;
    for line in code[within].split('\n') {
        ranges.push(at..at + line.len());
        at += line.len() + 1;
    }
    ranges
}

fn hash_line(code: &str, range: &Range<usize>) -> u64 {
    let mut hasher = DefaultHasher::new();
    "source".hash(&mut hasher);
    code[range.clone()].hash(&mut hasher);
    hasher.finish()
}

fn key(hash: u64, size: u32) -> u64 {
    let mut hasher = DefaultHasher::new();
    hash.hash(&mut hasher);
    size.hash(&mut hasher);
    hasher.finish()
}

fn guess(range: &Range<usize>, line: Pixels, indent: Pixels) -> Guess {
    Guess {
        chars: range.len(),
        line,
        rows: 0,
        extra: px(0.0),
        indent,
    }
}

/// `old` with `cut` replaced by `new`.
fn splice<T: Clone>(old: &[T], cut: Range<usize>, new: impl IntoIterator<Item = T>) -> Rc<[T]> {
    old[..cut.start]
        .iter()
        .cloned()
        .chain(new)
        .chain(old[cut.end..].iter().cloned())
        .collect()
}

/// Paint a document's own markdown source: a fence's caret, selection and hit
/// testing, without a fence's box, band or copy button.
///
/// The caret is a [`Cursor`] at block 0 in [`Part::Code`] — what a document
/// held as one fence answers to, which is how an editor holds its source.
/// Wrapping is not optional here: a paragraph is one line of markdown, and a
/// source view that scrolled sideways would hide most of it.
///
/// Set at [`Typography::body`]'s size and leading, in [`Theme::font_body`].
pub fn render_source(source: &SourceText, editing: Editing, cx: &mut App) -> AnyElement {
    let code: &str = &source.code;
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
    let mut theme = Theme::of(cx).clone();
    theme.font_mono = theme.font_body.clone();
    let mut typography = typography.unwrap_or_else(|| cx.typography());
    typography.code = typography.body;
    let overlay = Overlay {
        block: 0,
        part: Part::Code,
        selection,
        caret_on,
        caret_shape: cx.caret_shape(),
        caret_height: cx.caret_height(),
        caret_inactive: cx.inactive_caret(),
        // No window reaches here; a source view's rows read their own below.
        window_active: cx.active_window().is_some(),
        layouts,
        annotations,
        placeholder: None,
        caption: Caption::default(),
        // The source view is one fence and holds no task block.
        toggle: None,
        image: None,
        image_menu: None,
        image_overlay: None,
        image_overlay_corner: ImageOverlayCorner::BottomRight,
        fence: None,
        sizing: None,
        table_controls: false,
        // It paints no band, so there is nowhere for the button to float.
        copy: CopyButton::Hidden,
        base: None,
        highlight: crate::marks::highlight_paint_of(cx),
        find: crate::find::find_paint_of(cx),
        comment: crate::comment::comment_paint_of(cx),
        jump: None,
    };
    let style = cx.source_style();
    let count = source.ranges.len();
    let gutter = Gutter::new(&style, count, &typography, &theme);

    let Some(layouts) = layouts else {
        let (underlay, over, lines) = code_lines(
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
            .child(code_body(0, underlay, over, lines, &typography, true))
            .into_any_element();
    };

    // Built a line at a time by the same column the blocks are: a line is to
    // this text what a block is to a document.
    let spans = source.spans.clone();
    let ranges = source.ranges.clone();
    let mut kept: Vec<usize> = keep.to_vec();
    if let Some(selection) = selection {
        kept.push(source.line_of(selection.head.offset));
        kept.push(source.line_of(selection.anchor.offset));
    }
    let line = px(typography.code.line_height());
    let indent = px(gutter.width() + 2.0 * CODE_PADDING_X);
    let sized = source.sized(typography.code.size(), line, indent);
    layouts.prune(&sized.keys);
    let paint = RowPaint::new(&overlay, code, typography.code.size(), &theme);
    let code = source.code.clone();
    let sink = layouts.clone();
    let column = Column {
        layouts: layouts.clone(),
        keys: sized.keys,
        gaps: sized.gaps,
        guesses: sized.guesses,
        keep: kept,
        scroll: scroll.cloned(),
        item_of: {
            let ranges = ranges.clone();
            Rc::new(move |at| ranges.partition_point(|line| line.end < at.offset))
        },
        build: Box::new(move |index, window, _| {
            let span = ranges[index].clone();
            let paint = RowPaint {
                hollow: !window.is_window_active(),
                ..paint.clone()
            };
            let (styled, runs) =
                code_line(&code[span.clone()], span.start, spans.as_deref(), &theme);
            let layout = styled.layout().clone();
            let sink = sink.clone();
            let over = {
                let (span, layout, paint) = (span.clone(), layout.clone(), paint.clone());
                canvas(
                    |_, _, _| (),
                    move |_, _, window, _| paint.paint_over(&span, &layout, &runs, window),
                )
                .absolute()
                .size_full()
            };
            let underlay = canvas(
                |_, _, _| (),
                move |_, _, window, _| {
                    sink.record(
                        0,
                        Part::Code,
                        span.clone(),
                        layout.clone(),
                        Shown::default(),
                    );
                    paint.paint_under(&span, &layout, window);
                },
            )
            .absolute()
            .size_full();
            let text = div().relative().child(underlay).child(styled).child(over);
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
    let spans = crate::highlight::spans(cx, language, code).or_else(|| {
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
type Span = (Range<usize>, theme::HighlightKind);

/// One line of code, coloured by the spans over the text it came from, and
/// its runs.
///
/// Runs are measured within the line; spans are byte ranges over the whole
/// text, so every span is clipped to the line and rebased.
fn code_line(
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
struct RowPaint {
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
    fn new(overlay: &Overlay, code: &str, code_size: f32, theme: &Theme) -> Self {
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

    fn paint_under(&self, span: &Range<usize>, layout: &TextLayout, window: &mut Window) {
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
    fn paint_over(
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
        .children(
            (overlay.copy == CopyButton::Shown).then(|| copy_button(code, ix, theme, window, cx)),
        )
        .into_any_element()
}

/// The box a fence is drawn in, painted or as code.
pub fn fence_panel(theme: &Theme) -> gpui::Div {
    div()
        .rounded(px(Theme::panel_radius()))
        .bg(theme.ink(0.035))
        .border_1()
        .border_color(theme.border)
        .overflow_hidden()
        .relative()
}

/// The band across the top of a [`fence_panel`], where its label and copy
/// button sit.
pub fn fence_band(theme: &Theme) -> gpui::Div {
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

/// A copy button that owns its own feedback.
///
/// The state is the element's, not the caller's: a component library cannot ask
/// every host to thread a handler and a "which block is showing Copied" index
/// through its render tree just to put a button on a code block. It resets when
/// the pointer leaves, which needs no clock.
pub(crate) fn copy_button(
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
