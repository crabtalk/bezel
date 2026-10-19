//! Source mode.

use super::*;

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
        let spans = highlight_source(crate::fence::highlight::installed(cx), &code).map(Into::into);
        let ranges = split_lines(&code, 0..code.len());
        let hashes = ranges.iter().map(|range| hash_line(&code, range)).collect();
        Self {
            code,
            ranges: ranges.into(),
            hashes,
            spans,
            generation: 0,
            highlighter: crate::fence::highlight::generation(cx),
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
        (self.highlighter != crate::fence::highlight::generation(cx)).then(|| self.span_job(cx))
    }

    fn span_job(&mut self, cx: &App) -> SpanJob {
        self.generation += 1;
        self.highlighter = crate::fence::highlight::generation(cx);
        SpanJob {
            code: self.code.clone(),
            highlighter: crate::fence::highlight::installed(cx),
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
        highlight: crate::paint::marks::highlight_paint_of(cx),
        find: crate::paint::find::find_paint_of(cx),
        comment: crate::paint::comment::comment_paint_of(cx),
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
            let paint = paint.hollow(!window.is_window_active());
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
