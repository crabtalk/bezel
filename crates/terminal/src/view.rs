//! Terminal paint + input encoding.
//!
//! - the ANSI palette on the terminal background — `#090909` dark, `#fafafa`
//!   light — and the 256-color cube/grayscale resolution, both resolved per
//!   [`Appearance`];
//! - keystroke → PTY byte encoding (printables, control keys, arrows/nav
//!   escape sequences, Ctrl- combos, Alt prefixing);
//! - the 12 ms input coalescer and the 80 ms resize debounce constants (the
//!   host drives the timers; the buffer logic here is pure);
//! - [`TerminalElement`] — a custom gpui element that measures cell metrics
//!   from the real mono font (the "font probe"), reports the resulting
//!   cols×rows back to the host through its grid callback, and paints the
//!   grid: background quads for non-default cells, one `ShapedLine` per row
//!   (same font whatever the colors — paint never changes layout), and the
//!   cursor block.

use gpui::{
    App, Bounds, GlobalElementId, Hsla, KeyLayout, LayoutId, Modifiers, PaintQuad, Pixels, Point,
    ShapedLine, SharedString, Style, TextRun, Window, fill, font, outline, point, px, relative,
    size,
};

use theme::{Appearance, Theme};

use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, Instant},
};

use crate::emulator::{
    CellColor, CellSnapshot, CursorSnapshot, Emulator, Frame, KeyboardMode, MouseMode,
    MouseTracking, Side, Source,
};

mod images;
mod keys;
mod mouse;
mod palette;

pub use images::*;
pub use keys::*;
pub use mouse::*;
pub use palette::*;

/// Terminal font metrics (mono).
pub const TERM_FONT_SIZE: f32 = 13.0;
pub const TERM_LINE_HEIGHT: f32 = 18.0;
/// Inner padding of the grid area.
pub const TERM_PADDING: f32 = 12.0;

/// Keyboard input coalescing window before a PTY write flush.
pub const COALESCE_MS: u64 = 12;
/// Debounce for a PTY resize after viewport-driven size changes.
pub const RESIZE_DEBOUNCE_MS: u64 = 80;

// ---------------------------------------------------------------------------
// Input coalescer (pure buffer; the host owns the 12 ms timer)
// ---------------------------------------------------------------------------

/// Buffers keyboard bytes between flushes. `push` returns `true` exactly when
/// a flush timer should be scheduled (the buffer was empty), so at most one
/// timer is in flight per burst.
#[derive(Debug, Default)]
pub struct InputCoalescer {
    pending: Vec<u8>,
}

impl InputCoalescer {
    pub fn push(&mut self, bytes: &[u8]) -> bool {
        let was_empty = self.pending.is_empty();
        self.pending.extend_from_slice(bytes);
        was_empty && !self.pending.is_empty()
    }

    pub fn take(&mut self) -> Vec<u8> {
        std::mem::take(&mut self.pending)
    }

    pub fn is_empty(&self) -> bool {
        self.pending.is_empty()
    }
}

// ---------------------------------------------------------------------------
// Grid element
// ---------------------------------------------------------------------------

/// A grid snapshot handed to the paint element.
pub struct GridSnapshot {
    pub lines: Vec<Vec<CellSnapshot>>,
    pub cursor: Option<CursorSnapshot>,
    /// Kitty graphics on the visible grid. Empty for a terminal that has
    /// never been sent one, which is every terminal until something is.
    pub images: Vec<PlacedImage>,
}

/// An image on the grid, decoded and placed in cells.
#[derive(Clone)]
#[non_exhaustive]
pub struct PlacedImage {
    pub row: usize,
    pub col: usize,
    pub cols: u16,
    pub rows: u16,
    /// The emulator's [`crate::emulator::Placement::frame`].
    pub frame: Frame,
    pub source: Source,
    pub z: i32,
    /// The kitty image id, which orders images of equal `z`.
    pub id: u32,
    /// The frame to paint.
    pub image: Arc<gpui::RenderImage>,
    /// Which of the image's frames [`Self::image`] is, 0-based.
    pub frame_index: usize,
    /// When the next frame of an animation is due, if one is.
    pub next_frame: Option<Instant>,
    /// Shared by every image one [`Images`] resolved: the element arms one
    /// repaint at a time through it.
    pub wake: Wake,
}

/// Where the grid landed this frame, in window coordinates.
///
/// Reported by element prepaint because that is the only place the measured
/// font metrics exist. Mouse events arrive on the wrapping div in window
/// space, so mapping a pointer to a cell needs the glyph origin and the cell
/// size the *current* frame used — a stale one puts the selection a row off
/// after a resize.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GridGeometry {
    /// Top-left of the first glyph (bounds origin plus padding).
    pub origin: Point<Pixels>,
    pub cell_w: f32,
    pub line_h: f32,
    pub cols: u16,
    pub rows: u16,
}

/// The host's grid hook: receives the measured grid placement, returns the
/// snapshot to paint.
type GridHook = Box<dyn Fn(GridGeometry, &mut App) -> Option<GridSnapshot>>;

/// Paints the host's grid. Cell metrics come from the resolved mono font each
/// frame (font probe): `em_advance` for the cell width, the fixed line height
/// for rows. The measured cols×rows are handed to the `grid` callback, which
/// resizes the emulator immediately, debounces the PTY resize, and returns the
/// snapshot to paint — one callback so the host borrows its own state exactly
/// once per frame.
pub struct TerminalElement {
    grid: GridHook,
    focused: bool,
    font_size: f32,
}

impl TerminalElement {
    pub fn new(
        grid: impl Fn(GridGeometry, &mut App) -> Option<GridSnapshot> + 'static,
        focused: bool,
    ) -> Self {
        Self {
            grid: Box::new(grid),
            focused,
            font_size: TERM_FONT_SIZE,
        }
    }

    /// Set the grid's font size in points, scaling the line height with it.
    /// Measurement, selection, cursor and paint all use these same metrics.
    pub fn with_text_size(mut self, points: f32) -> Self {
        if points.is_finite() && points > 0. {
            self.font_size = points;
        }
        self
    }

    fn line_height(&self) -> f32 {
        self.font_size * TERM_LINE_HEIGHT / TERM_FONT_SIZE
    }
}

/// Which pass of the grid's paint an image goes in, by its z-index.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Layer {
    UnderBackgrounds,
    UnderText,
    OverText,
}

impl Layer {
    fn of(z: i32) -> Self {
        match z {
            z if z < i32::MIN / 2 => Layer::UnderBackgrounds,
            z if z < 0 => Layer::UnderText,
            _ => Layer::OverText,
        }
    }
}

/// An image as prepaint resolved it.
struct Painted {
    layer: Layer,
    /// `(z, id)`: lower paints first.
    order: (i32, u32),
    /// The placement's frame, cut to its own cells.
    clip: Bounds<Pixels>,
    /// The whole image, positioned so its source rectangle fills the frame.
    whole: Bounds<Pixels>,
    image: Arc<gpui::RenderImage>,
}

pub struct TerminalPrepaint {
    bg_quads: Vec<PaintQuad>,
    /// Each visible image, in paint order.
    images: Vec<Painted>,
    /// Selection wash. Painted after [`Self::bg_quads`] and before the glyphs:
    /// it has to tint a cell's own background rather than replace it, and it
    /// must not wash out the text it is highlighting.
    sel_quads: Vec<PaintQuad>,
    /// Per row, the shaped segments and the grid COLUMN each one starts at.
    /// Not one line per row: see [`shape_row`].
    lines: Vec<Vec<(usize, ShapedLine)>>,
    /// Grid cell advance, so paint can place segments by column.
    cell_w: Pixels,
    cursor: Option<PaintQuad>,
}

impl gpui::IntoElement for TerminalElement {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}

impl gpui::Element for TerminalElement {
    type RequestLayoutState = ();
    type PrepaintState = TerminalPrepaint;

    fn id(&self) -> Option<gpui::ElementId> {
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
    ) -> (LayoutId, Self::RequestLayoutState) {
        let mut style = Style::default();
        style.size.width = relative(1.0).into();
        style.size.height = relative(1.0).into();
        (window.request_layout(style, [], cx), ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&gpui::InspectorElementId>,
        bounds: Bounds<Pixels>,
        _state: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        let theme = Theme::of(cx).clone();
        // Ligatures OFF. A terminal is a fixed grid: the shaper must emit one
        // cell-width advance per character, and a contextual substitution
        // (Geist Mono ligates `--`, `->`, …) collapses several cells into
        // fewer glyphs, so the row renders SHORT while the cursor — a quad at
        // `cell_w * col` — stays on the true column. That is the `codex
        // --yolo` → `codex--yolo` report: the space is in the grid and went
        // to the pty (the command runs), only the painted run lost a cell.
        // The landing page disables the same three features on its ASCII art
        // for the same reason.
        let mut mono = font(theme.font_mono.clone());
        mono.features = gpui::FontFeatures(std::sync::Arc::new(vec![
            ("liga".into(), 0),
            ("calt".into(), 0),
            ("dlig".into(), 0),
        ]));
        // Font probe: measure the actual advance of the resolved mono font so
        // cols/rows track real glyph metrics, not a guessed aspect ratio.
        let font_size = px(self.font_size);
        let font_id = window.text_system().resolve_font(&mono);
        let cell_w = window
            .text_system()
            .em_advance(font_id, font_size)
            .unwrap_or(px(self.font_size * 0.6));
        let line_h = px(self.line_height());

        let inner_w = f32::from(bounds.size.width) - 2.0 * TERM_PADDING;
        let inner_h = f32::from(bounds.size.height) - 2.0 * TERM_PADDING;
        let cols = ((inner_w / f32::from(cell_w)).floor() as i64).clamp(2, 500) as u16;
        let rows = ((inner_h / f32::from(line_h)).floor() as i64).clamp(1, 500) as u16;

        // Report the measured grid, then snapshot for painting.
        let origin = point(
            bounds.left() + px(TERM_PADDING),
            bounds.top() + px(TERM_PADDING),
        );
        let snapshot = (self.grid)(
            GridGeometry {
                origin,
                cell_w: f32::from(cell_w),
                line_h: f32::from(line_h),
                cols,
                rows,
            },
            cx,
        );
        let Some(snapshot) = snapshot else {
            return TerminalPrepaint {
                bg_quads: Vec::new(),
                images: Vec::new(),
                sel_quads: Vec::new(),
                lines: Vec::new(),
                cell_w,
                cursor: None,
            };
        };

        let next = snapshot
            .images
            .iter()
            .filter_map(|placed| Some((placed.next_frame?, &placed.wake)))
            .min_by_key(|(due, _)| *due);
        if let Some((due, wake)) = next
            && wake.arm(due)
        {
            let wake = wake.clone();
            let view = window.current_view();
            let delay = due.saturating_duration_since(Instant::now());
            cx.spawn(async move |cx| {
                cx.background_executor().timer(delay).await;
                wake.fired(due);
                cx.update(|cx| cx.notify(view));
            })
            .detach();
        }
        // Cells to pixels, at the metrics this frame measured — the same
        // arithmetic the cursor quad uses, so an image sits on the grid rather
        // than near it.
        let mut images: Vec<Painted> = snapshot
            .images
            .iter()
            .map(|placed| {
                let frame = Bounds::new(
                    point(
                        origin.x + cell_w * (placed.col as f32 + placed.frame.x),
                        origin.y + line_h * (placed.row as f32 + placed.frame.y),
                    ),
                    size(cell_w * placed.frame.width, line_h * placed.frame.height),
                );
                // The whole image at the scale that maps the source onto the
                // frame; the frame is the clip.
                let scale_x = frame.size.width / placed.source.width as f32;
                let scale_y = frame.size.height / placed.source.height as f32;
                let natural = placed.image.size(0);
                let whole = Bounds::new(
                    point(
                        frame.origin.x - scale_x * placed.source.x as f32,
                        frame.origin.y - scale_y * placed.source.y as f32,
                    ),
                    size(
                        scale_x * natural.width.0 as f32,
                        scale_y * natural.height.0 as f32,
                    ),
                );
                let cells = Bounds::new(
                    point(
                        origin.x + cell_w * placed.col as f32,
                        origin.y + line_h * placed.row as f32,
                    ),
                    size(cell_w * placed.cols as f32, line_h * placed.rows as f32),
                );
                Painted {
                    layer: Layer::of(placed.z),
                    order: (placed.z, placed.id),
                    clip: frame.intersect(&cells),
                    whole,
                    image: placed.image.clone(),
                }
            })
            .collect();
        images.sort_by_key(|painted| painted.order);

        let mut bg_quads = Vec::new();
        let mut sel_quads = Vec::new();
        let mut lines = Vec::with_capacity(snapshot.lines.len());

        for (row_ix, row) in snapshot.lines.iter().enumerate() {
            let y = origin.y + line_h * row_ix as f32;
            // Selected runs, merged the same way background runs are: one quad
            // per contiguous span instead of one per cell.
            let mut sel_start: Option<usize> = None;
            for col in 0..=row.len() {
                let selected = row.get(col).is_some_and(|cell| cell.selected);
                match (sel_start, selected) {
                    (None, true) => sel_start = Some(col),
                    (Some(start), false) => {
                        sel_quads.push(fill(
                            Bounds::new(
                                point(origin.x + cell_w * start as f32, y),
                                size(cell_w * (col - start) as f32, line_h),
                            ),
                            terminal_selection_for(theme.appearance),
                        ));
                        sel_start = None;
                    }
                    _ => {}
                }
            }
            // Merge consecutive non-default background cells into quads.
            let mut run_start: Option<(usize, Hsla)> = None;
            for (col, color) in row
                .iter()
                .map(|cell| cell.display_colors().1)
                .chain(std::iter::once(CellColor::Background))
                .enumerate()
            {
                let paint = match color {
                    CellColor::Background => None,
                    other => Some(resolve_color(other, &theme)),
                };
                match (&run_start, paint) {
                    (None, Some(color)) => run_start = Some((col, color)),
                    (Some((start, current)), next) if next != Some(*current) => {
                        bg_quads.push(fill(
                            Bounds::new(
                                point(origin.x + cell_w * *start as f32, y),
                                size(cell_w * (col - *start) as f32, line_h),
                            ),
                            *current,
                        ));
                        run_start = next.map(|color| (col, color));
                    }
                    _ => {}
                }
            }
            lines.push(shape_row(row, &theme, &mono, font_size, window));
        }

        let cursor = snapshot.cursor.map(|c| {
            let cursor_bounds = Bounds::new(
                point(
                    origin.x + cell_w * c.col as f32,
                    origin.y + line_h * c.row as f32,
                ),
                size(cell_w, line_h),
            );
            if self.focused {
                // Translucent block: the glyph underneath stays legible.
                fill(cursor_bounds, theme.cursor)
            } else {
                outline(cursor_bounds, theme.cursor, gpui::BorderStyle::Solid)
            }
        });

        TerminalPrepaint {
            bg_quads,
            images,
            sel_quads,
            lines,
            cell_w,
            cursor,
        }
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&gpui::InspectorElementId>,
        bounds: Bounds<Pixels>,
        _state: &mut Self::RequestLayoutState,
        prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        let line_h = px(self.line_height());
        let origin = point(
            bounds.left() + px(TERM_PADDING),
            bounds.top() + px(TERM_PADDING),
        );
        window.with_content_mask(Some(gpui::ContentMask::new(bounds)), |window| {
            let images = std::mem::take(&mut prepaint.images);
            let paint_layer = |layer: Layer, window: &mut Window| {
                for painted in images.iter().filter(|painted| painted.layer == layer) {
                    let _ = window.paint_image(
                        painted.clip,
                        painted.whole,
                        gpui::Corners::default(),
                        painted.image.clone(),
                        0,
                        false,
                    );
                }
            };
            paint_layer(Layer::UnderBackgrounds, window);
            for quad in prepaint.bg_quads.drain(..) {
                window.paint_quad(quad);
            }
            for quad in prepaint.sel_quads.drain(..) {
                window.paint_quad(quad);
            }
            paint_layer(Layer::UnderText, window);
            let cell_w = prepaint.cell_w;
            for (ix, segments) in prepaint.lines.iter().enumerate() {
                let y = origin.y + line_h * ix as f32;
                for (col, line) in segments {
                    let _ = line.paint(
                        point(origin.x + cell_w * *col as f32, y),
                        line_h,
                        gpui::TextAlign::Left,
                        None,
                        window,
                        cx,
                    );
                }
            }
            paint_layer(Layer::OverText, window);
            if let Some(cursor) = prepaint.cursor.take() {
                window.paint_quad(cursor);
            }
        });
    }
}

/// Shape one grid row into COLUMN-PINNED segments.
///
/// A terminal is a fixed grid, but a shaped line places glyphs by their font
/// advances. Those agree only while every glyph is monospace-width — the row's
/// `cell_w` IS the mono font's em advance. The moment a glyph resolves through
/// FONT FALLBACK (box drawing `│─╭`, arrows `→`, emoji, CJK) its advance is
/// whatever that other font uses, and the whole rest of the line slides out of
/// the grid: box borders land a few pixels off (user report: "one of the pipes
/// is broken"), and a double-width glyph whose fallback advances only one cell
/// swallows the column after it (user report: `codex --yolo` rendering as
/// `codex--yolo`). Backgrounds, selection and the cursor never drifted because
/// those are quads placed at `cell_w * col`.
///
/// So: runs of ASCII shape together (guaranteed cell-width in a mono font),
/// and every other glyph is its own segment pinned at its own column. Wide
/// spacers are still skipped — the wide glyph covers both columns, and the
/// NEXT segment re-pins regardless.
fn shape_row(
    row: &[CellSnapshot],
    theme: &Theme,
    mono: &gpui::Font,
    font_size: Pixels,
    window: &Window,
) -> Vec<(usize, ShapedLine)> {
    fn flush(
        segments: &mut Vec<(usize, ShapedLine)>,
        text: &mut String,
        runs: &mut Vec<TextRun>,
        seg_col: usize,
        font_size: Pixels,
        window: &Window,
    ) {
        if text.is_empty() {
            return;
        }
        let shaped = window.text_system().shape_line(
            SharedString::from(std::mem::take(text)),
            font_size,
            runs,
            None,
        );
        segments.push((seg_col, shaped));
        runs.clear();
    }

    let mut segments: Vec<(usize, ShapedLine)> = Vec::new();
    let mut text = String::with_capacity(row.len());
    let mut runs: Vec<TextRun> = Vec::new();
    let mut seg_col = 0usize;

    for (col, cell) in row.iter().enumerate() {
        if cell.wide_spacer {
            continue;
        }
        // Tab stops are already expanded in the grid. Paint the stored tab
        // marker as one blank cell so the shaper cannot expand it again.
        let ch = if cell.hidden || cell.ch == '\t' {
            ' '
        } else {
            cell.ch
        };
        // Anything that can leave the mono font gets its own pinned segment.
        let pinned = !ch.is_ascii() || cell.wide;
        if pinned {
            flush(
                &mut segments,
                &mut text,
                &mut runs,
                seg_col,
                font_size,
                window,
            );
        }
        if text.is_empty() {
            seg_col = col;
        }
        let (fg, _) = cell.display_colors();
        let mut color = resolve_color(fg, theme);
        if cell.dim {
            color.a *= 0.6;
        }
        let mut cell_font = mono.clone();
        cell_font.weight = if cell.bold {
            gpui::FontWeight::BOLD
        } else {
            gpui::FontWeight::NORMAL
        };
        cell_font.style = if cell.italic {
            gpui::FontStyle::Italic
        } else {
            gpui::FontStyle::Normal
        };
        let underline = cell.underline.then_some(gpui::UnderlineStyle {
            color: Some(color),
            thickness: px(1.0),
            wavy: false,
        });
        let len = ch.len_utf8();
        text.push(ch);
        match runs.last_mut() {
            Some(last)
                if last.color == color && last.font == cell_font && last.underline == underline =>
            {
                last.len += len;
            }
            _ => runs.push(TextRun {
                len,
                font: cell_font,
                color,
                background_color: None,
                underline,
                strikethrough: None,
            }),
        }
        if pinned {
            flush(
                &mut segments,
                &mut text,
                &mut runs,
                seg_col,
                font_size,
                window,
            );
        }
    }
    flush(
        &mut segments,
        &mut text,
        &mut runs,
        seg_col,
        font_size,
        window,
    );
    segments
}
