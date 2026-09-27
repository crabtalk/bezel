//! Image placements: kitty, sixel and iTerm2 on the grid, and their deletes.

use super::*;

impl Emulator {
    /// The images the client has sent, by id.
    pub fn graphics(&self) -> &kitty::Store {
        &self.graphics
    }

    /// Answer DA1 as a VT220 with sixel graphics (`CSI ? 62 ; 4 ; 22 c`)
    /// rather than as a VT102 (`CSI ? 6 c`). Off until a host turns it on.
    /// Programs read attribute 4 in that answer to decide whether to send
    /// sixel images; they are shown either way.
    pub fn set_advertise_sixel(&mut self, advertise: bool) {
        self.advertise_sixel = advertise;
    }

    /// See [`kitty::Store::set_local_media`].
    pub fn set_local_media(&mut self, allow: bool) {
        self.graphics.set_local_media(allow);
    }

    /// Put an image on the grid at the cursor, and move the cursor past it
    /// unless the command said not to.
    ///
    /// The rows it covers are reserved by feeding that many linefeeds through
    /// the parser rather than by moving the cursor directly: a linefeed at the
    /// bottom of the screen scrolls, which is what pushes the image's own
    /// anchors into history at the same moment its text would have gone.
    /// `C=1` reserves nothing, so the image covers whatever is drawn under it.
    ///
    /// A placement replaces the one the image already has under the same
    /// placement id, zero included: an image placed with no `p` has at most
    /// one placement.
    pub(super) fn place(&mut self, display: kitty::Display) -> Result<(), &'static str> {
        let image = display.image;
        let key = (image, display.placement);
        if display.parent_image != 0 {
            return self.relate(display);
        }
        let Some((cols, rows, frame, source)) = self.extent(&display) else {
            return Ok(());
        };
        self.relatives.remove(&key);
        if display.unicode {
            let virtual_ = Virtual {
                cols: cols.max(1.0).min(u16::MAX as f32) as u16,
                rows: rows.max(1.0).min(u16::MAX as f32) as u16,
                frame,
                source,
                z: display.z,
            };
            self.virtuals.insert(key, virtual_);
            return Ok(());
        }
        let cols = (cols as usize).clamp(1, self.cols()) as u16;
        let rows = (rows as usize).clamp(1, self.rows()) as u16;

        self.anchor(display, cols, rows, frame, source);
        Ok(())
    }

    /// Show a sixel image at the cursor, the way `a=T` shows one.
    pub(super) fn sixel(&mut self, data: &[u8]) {
        let Some(rgba) = crate::sixel::decode(data) else {
            return;
        };
        let image = kitty::Image::still(kitty::Format::Rgba, rgba.width, rgba.height, rgba.bytes);
        let id = self.graphics.hold(image);
        let _ = self.place(kitty::Display::at_cursor(id));
    }

    /// An iTerm2 file command: a whole file, or a part of one.
    pub(super) fn iterm(&mut self, command: Iterm) {
        match command {
            Iterm::File { args, payload } => {
                self.iterm_show(&args, &kitty::decode(&payload));
            }
            Iterm::Begin { args } => self.multipart = Some((args, Vec::new())),
            Iterm::Part(payload) => {
                let Some((_, bytes)) = &mut self.multipart else {
                    return;
                };
                bytes.extend(kitty::decode(&payload));
                if bytes.len() > crate::iterm::MAX_FILE {
                    self.multipart = None;
                }
            }
            Iterm::End => {
                if let Some((args, bytes)) = self.multipart.take() {
                    self.iterm_show(&args, &bytes);
                }
            }
        }
    }

    /// Show an iTerm2 file at the cursor, if it is an inline image.
    pub(super) fn iterm_show(&mut self, args: &[u8], bytes: &[u8]) {
        let args = crate::iterm::Args::parse(args);
        if !args.inline || bytes.len() > crate::iterm::MAX_FILE {
            return;
        }
        let Some(image) = crate::iterm::image(bytes) else {
            return;
        };
        let id = self.graphics.hold(image);
        let mut display = kitty::Display::at_cursor(id);
        if let Some((cell_w, cell_h)) = self.cell {
            display.columns = args.width.cells(cell_w, self.cols());
            display.rows = args.height.cells(cell_h, self.rows());
        }
        display.stretch = !args.preserve_aspect;
        let _ = self.place(display);
    }

    /// Hang a placement off the parent its `P` and `Q` name. The cursor stays
    /// where it is, whatever `C` says.
    pub(super) fn relate(&mut self, display: kitty::Display) -> Result<(), &'static str> {
        let key = (display.image, display.placement);
        let parent = (display.parent_image, display.parent_placement);
        if display.unicode {
            return Err("EINVAL:a virtual placement cannot be relative");
        }
        if !self.exists(parent) {
            return Err("ENOPARENT");
        }
        let mut depth = 1;
        let mut at = parent;
        loop {
            if at == key {
                return Err("ECYCLE");
            }
            match self.relatives.get(&at) {
                Some(relative) => {
                    depth += 1;
                    at = relative.parent;
                }
                None => break,
            }
        }
        if depth > MAX_RELATIVE_DEPTH {
            return Err("ETOODEEP");
        }
        let Some((cols, rows, frame, source)) = self.extent(&display) else {
            return Ok(());
        };
        self.placed
            .retain(|_, placed| (placed.image, placed.placement) != key);
        self.virtuals.remove(&key);
        self.relatives.insert(
            key,
            Relative {
                parent,
                offset: (display.parent_offset_x, display.parent_offset_y),
                cols: cols.clamp(1.0, u16::MAX as f32) as u16,
                rows: rows.clamp(1.0, u16::MAX as f32) as u16,
                frame,
                source,
                z: display.z,
            },
        );
        Ok(())
    }

    /// Whether any placement, of any kind, goes by `key`.
    pub(super) fn exists(&self, key: (u32, u32)) -> bool {
        self.placed
            .values()
            .any(|placed| (placed.image, placed.placement) == key)
            || self.virtuals.contains_key(&key)
            || self.relatives.contains_key(&key)
    }

    /// Whether any placement, of any kind, shows `image`.
    pub(super) fn shown(&self, image: u32) -> bool {
        self.placed.values().any(|placed| placed.image == image)
            || self.virtuals.keys().any(|&(held, _)| held == image)
            || self.relatives.keys().any(|&(held, _)| held == image)
    }

    /// The cells a display covers, unclamped, with its frame and source.
    /// `None` until a cell has been measured, and for an image with nothing
    /// to show.
    pub(super) fn extent(&self, display: &kitty::Display) -> Option<(f32, f32, Frame, Source)> {
        let (cell_w, cell_h) = self.cell?;
        let (width, height) = self
            .graphics
            .get(display.image)
            .and_then(kitty::Image::size)?;
        let x = display.source_x.min(width);
        let y = display.source_y.min(height);
        let source = Source {
            x,
            y,
            width: match display.source_width {
                0 => width - x,
                w => w.min(width - x),
            },
            height: match display.source_height {
                0 => height - y,
                h => h.min(height - y),
            },
        };
        if source.width == 0 || source.height == 0 {
            return None;
        }
        let (source_w, source_h) = (source.width as f32, source.height as f32);
        let offset_x = (display.offset_x as f32).min(cell_w - 1.0).max(0.0);
        let offset_y = (display.offset_y as f32).min(cell_h - 1.0).max(0.0);
        // The frame in pixels, and the cells it covers. Unscaled without `c`
        // and `r`; one of them scales the other by the source's aspect; both
        // fit the source inside the box they make, centred. The offset counts
        // toward the cells only when neither is given.
        let (cols, rows, frame) = match (display.columns, display.rows) {
            (0, 0) => (
                ((offset_x + source_w) / cell_w).ceil(),
                ((offset_y + source_h) / cell_h).ceil(),
                (offset_x, offset_y, source_w, source_h),
            ),
            (c, 0) => {
                let w = c as f32 * cell_w;
                let h = w * source_h / source_w;
                (c as f32, (h / cell_h).ceil(), (offset_x, offset_y, w, h))
            }
            (0, r) => {
                let h = r as f32 * cell_h;
                let w = h * source_w / source_h;
                ((w / cell_w).ceil(), r as f32, (offset_x, offset_y, w, h))
            }
            (c, r) if display.stretch => {
                let (w, h) = (c as f32 * cell_w, r as f32 * cell_h);
                (c as f32, r as f32, (offset_x, offset_y, w, h))
            }
            (c, r) => {
                let (box_w, box_h) = (c as f32 * cell_w, r as f32 * cell_h);
                let scale = (box_w / source_w).min(box_h / source_h);
                let (w, h) = (source_w * scale, source_h * scale);
                let x = offset_x + (box_w - w) / 2.0;
                let y = offset_y + (box_h - h) / 2.0;
                (c as f32, r as f32, (x, y, w, h))
            }
        };
        let frame = Frame {
            x: frame.0 / cell_w,
            y: frame.1 / cell_h,
            width: frame.2 / cell_w,
            height: frame.3 / cell_h,
        };
        Some((cols, rows, frame, source))
    }

    /// Write a placement's anchors at the cursor, and move the cursor past it
    /// unless the display said not to.
    pub(super) fn anchor(
        &mut self,
        display: kitty::Display,
        cols: u16,
        rows: u16,
        frame: Frame,
        source: Source,
    ) {
        let image = display.image;
        self.placed
            .retain(|_, placed| placed.image != image || placed.placement != display.placement);
        let anchor = self.next_anchor % ANCHOR_MAX;
        self.next_anchor = anchor.wrapping_add(1);
        // The id is reused once the ring wraps, so whatever wore it last stops
        // being a placement before the new one starts.
        self.placed.remove(&anchor);

        let cursor = self.term.grid().cursor.point;
        let width = (cols as usize)
            .min(ANCHOR_COLUMN_MAX as usize)
            .min(self.cols() - cursor.column.0);
        for offset in 0..width {
            let cell = &mut self.term.grid_mut()[cursor.line][Column(cursor.column.0 + offset)];
            // A cell only ever gains zerowidth chars, so one anchored again
            // without being written to in between would collect a pair per
            // redraw. Rebuilt from the marks still worth keeping — the pairs
            // of placements still live — which takes this cell's underline
            // color and hyperlink with it.
            if cell
                .zerowidth()
                .is_some_and(|marks| marks.iter().any(|&mark| is_anchor(mark)))
            {
                let marks = cell.zerowidth().unwrap_or(&[]);
                let mut kept: Vec<char> = marks
                    .iter()
                    .copied()
                    .filter(|&mark| !is_anchor(mark))
                    .collect();
                for (live, column) in anchor_pairs(marks) {
                    if !self.placed.contains_key(&live) {
                        continue;
                    }
                    kept.extend(char::from_u32(ANCHOR + live));
                    kept.extend(
                        column.and_then(|column| char::from_u32(ANCHOR_COLUMN + column as u32)),
                    );
                }
                cell.extra = None;
                for mark in kept {
                    cell.push_zerowidth(mark);
                }
            }
            for mark in [ANCHOR + anchor, ANCHOR_COLUMN + offset as u32] {
                if let Some(mark) = char::from_u32(mark) {
                    cell.push_zerowidth(mark);
                }
            }
        }
        self.placed.insert(
            anchor,
            Placed {
                image,
                placement: display.placement,
                cols,
                rows,
                frame,
                source,
                z: display.z,
            },
        );
        if display.cursor_movement == kitty::CursorMovement::After {
            for _ in 0..rows {
                self.parser.advance(&mut self.term, b"\n");
            }
        }
    }

    /// Every image on the visible grid, found by the anchors the cells carry.
    ///
    /// Walked per frame rather than cached: an anchor moves with its cell, and
    /// nothing tells us when. Each placement is taken from the first of its
    /// anchors the walk reaches, whose own column is what puts the image's
    /// left edge back; a placement with no anchor left is one whose whole top
    /// row was overwritten, which is what clearing the screen does to it.
    pub fn placements(&self) -> Vec<Placement> {
        match &self.held {
            Some(held) => held.placements.clone(),
            None => self.live_placements(),
        }
    }

    pub(super) fn live_placements(&self) -> Vec<Placement> {
        let located = self.locate(self.display_offset() as i32);
        let mut out: Vec<Placement> = located.anchored.into_iter().map(|(_, p)| p).collect();
        out.extend(located.pieces.into_iter().map(|(_, p)| p));
        out.extend(located.relatives.into_iter().map(|(_, p)| p));
        out
    }

    /// Every placement on the rows `offset` lines above the bottom of the
    /// screen, each with what names it.
    ///
    /// A relative placement sits where its parent is found on those rows, so
    /// one whose parent is off them is not found either. A virtual parent is
    /// at the least row and least column of the placeholder cells showing it.
    pub(super) fn locate(&self, offset: i32) -> Located {
        let (anchored, pieces) = self.scan(offset);
        let mut relatives = Vec::new();
        if !self.relatives.is_empty() {
            let mut at: std::collections::HashMap<(u32, u32), (i64, i64)> =
                std::collections::HashMap::new();
            for (anchor, p) in &anchored {
                if let Some(placed) = self.placed.get(anchor) {
                    at.insert(
                        (placed.image, placed.placement),
                        (p.row as i64, p.col as i64),
                    );
                }
            }
            for (key, p) in &pieces {
                let spot = at.entry(*key).or_insert((p.row as i64, p.col as i64));
                *spot = (spot.0.min(p.row as i64), spot.1.min(p.col as i64));
            }
            for (&key, relative) in &self.relatives {
                let Some((row, col)) = self.relative_spot(key, &at) else {
                    continue;
                };
                if row < 0 || col < 0 || row >= self.rows() as i64 || col >= self.cols() as i64 {
                    continue;
                }
                relatives.push((
                    key,
                    Placement {
                        row: row as usize,
                        col: col as usize,
                        cols: relative.cols,
                        rows: relative.rows,
                        image: key.0,
                        frame: relative.frame,
                        source: relative.source,
                        z: relative.z,
                    },
                ));
            }
        }
        Located {
            anchored,
            pieces,
            relatives,
        }
    }

    /// Where a relative placement's top-left cell is, walking its chain up
    /// to a parent found on the grid.
    pub(super) fn relative_spot(
        &self,
        key: (u32, u32),
        at: &std::collections::HashMap<(u32, u32), (i64, i64)>,
    ) -> Option<(i64, i64)> {
        let mut row = 0;
        let mut col = 0;
        let mut key = key;
        for _ in 0..=MAX_RELATIVE_DEPTH {
            let Some(relative) = self.relatives.get(&key) else {
                return at.get(&key).map(|&(r, c)| (r + row, c + col));
            };
            row += relative.offset.1 as i64;
            col += relative.offset.0 as i64;
            key = relative.parent;
        }
        None
    }

    /// The placements on the rows `offset` lines above the bottom of the
    /// screen, in one walk of their cells: anchored ones by anchor id, and the
    /// slices of virtual placements that placeholder cells show, one per run
    /// of cells that continue each other along a row. Zero is the screen the
    /// cursor moves on.
    pub(super) fn scan(&self, offset: i32) -> (Found<u32>, Found<(u32, u32)>) {
        let mut anchored: Vec<(u32, Placement)> = Vec::new();
        let mut seen = std::collections::HashSet::new();
        let mut pieces: Vec<((u32, u32), Placement)> = Vec::new();
        let grid = self.term.grid();
        for row in 0..self.rows() {
            let line = Line(row as i32 - offset);
            let mut decoder =
                (!self.virtuals.is_empty()).then(crate::placeholder::RowDecoder::default);
            // The run being grown: the key it was resolved to, the slot of its
            // last cell, and its index in `pieces`.
            let mut run: Option<((u32, u32), crate::placeholder::Slot, usize)> = None;
            for col in 0..self.cols() {
                let cell = &grid[line][Column(col)];
                let marks = cell.zerowidth().unwrap_or(&[]);
                for (anchor, within) in anchor_pairs(marks) {
                    let Some(placed) = self.placed.get(&anchor) else {
                        continue;
                    };
                    if !seen.insert(anchor) {
                        continue;
                    }
                    // A cell whose column mark was lost still holds the image;
                    // it can only say the left edge is here.
                    let within = within.unwrap_or(0);
                    anchored.push((
                        anchor,
                        Placement {
                            row,
                            col: col.saturating_sub(within),
                            cols: placed.cols,
                            rows: placed.rows,
                            image: placed.image,
                            frame: placed.frame,
                            source: placed.source,
                            z: placed.z,
                        },
                    ));
                }
                let Some(decoder) = decoder.as_mut() else {
                    continue;
                };
                let shown = decoder
                    .cell(cell)
                    .and_then(|slot| self.shown_by(slot).map(|(key, v)| (slot, key, v)));
                let Some((slot, key, virtual_)) = shown else {
                    run = None;
                    continue;
                };
                if let Some((run_key, last, at)) = &mut run
                    && *run_key == key
                    && last.row == slot.row
                    && last.col + 1 == slot.col
                {
                    pieces[*at].1.cols += 1;
                    *last = slot;
                    continue;
                }
                run = Some((key, slot, pieces.len()));
                pieces.push((
                    key,
                    Placement {
                        row,
                        col,
                        cols: 1,
                        rows: 1,
                        image: key.0,
                        frame: Frame {
                            x: virtual_.frame.x - slot.col as f32,
                            y: virtual_.frame.y - slot.row as f32,
                            ..virtual_.frame
                        },
                        source: virtual_.source,
                        z: virtual_.z,
                    },
                ));
            }
        }
        (anchored, pieces)
    }

    /// The virtual placement a placeholder cell shows, and its key. A cell
    /// naming no placement id shows the image's virtual placement with the
    /// lowest id. A cell outside its placement's box shows nothing.
    pub(super) fn shown_by(
        &self,
        slot: crate::placeholder::Slot,
    ) -> Option<((u32, u32), &Virtual)> {
        let key = match slot.placement {
            0 => self
                .virtuals
                .keys()
                .filter(|(image, _)| *image == slot.image)
                .min()
                .copied()?,
            placement => (slot.image, placement),
        };
        let virtual_ = self.virtuals.get(&key)?;
        (slot.row < virtual_.rows as u32 && slot.col < virtual_.cols as u32)
            .then_some((key, virtual_))
    }

    /// Take the placements a delete names off the grid, and with an
    /// upper-case one, free the data of each image it leaves with none.
    ///
    /// The ones named by position — every target but an image id, a range of
    /// them and a z-index — are looked for on the screen alone, not in
    /// history.
    pub(super) fn delete(&mut self, delete: kitty::Delete) {
        use kitty::Target;
        if delete.target == Target::All && delete.free {
            self.placed.clear();
            self.relatives.clear();
            self.graphics.clear();
            return;
        }
        let covers = |p: &Placement, col: u32, row: u32| {
            let (col, row) = (col as usize, row as usize);
            (p.col..p.col + p.cols as usize).contains(&col)
                && (p.row..p.row + p.rows as usize).contains(&row)
        };
        // Which keys a delete names: an image id and placement, a range of
        // ids, or a z-index. `None` for the deletes that name positions.
        let names = |image: u32, placement: u32, z: i32| match delete.target {
            Target::Image { id, placement: p } => Some(image == id && (p == 0 || placement == p)),
            Target::Range(low, high) => Some((low..=high).contains(&image)),
            Target::Z(want) => Some(z == want),
            _ => None,
        };
        let by_name = matches!(
            delete.target,
            Target::Image { .. } | Target::Range(..) | Target::Z(_)
        );
        let mut anchors: Vec<u32> = Vec::new();
        let mut keys: Vec<(u32, u32)> = Vec::new();
        if by_name {
            anchors.extend(
                self.placed
                    .iter()
                    .filter(|(_, p)| names(p.image, p.placement, p.z) == Some(true))
                    .map(|(&anchor, _)| anchor),
            );
            keys.extend(
                self.relatives
                    .iter()
                    .filter(|(key, r)| names(key.0, key.1, r.z) == Some(true))
                    .map(|(&key, _)| key),
            );
        } else {
            let cursor = self.term.grid().cursor.point;
            let hit = |p: &Placement| match delete.target {
                Target::Cursor => covers(p, cursor.column.0 as u32, cursor.line.0 as u32),
                Target::Cell { col, row, z } => covers(p, col, row) && z.is_none_or(|z| p.z == z),
                Target::Column(col) => (p.col..p.col + p.cols as usize).contains(&(col as usize)),
                Target::Row(row) => (p.row..p.row + p.rows as usize).contains(&(row as usize)),
                _ => true,
            };
            let located = self.locate(0);
            anchors.extend(
                located
                    .anchored
                    .iter()
                    .filter(|(_, p)| hit(p))
                    .map(|(a, _)| *a),
            );
            keys.extend(
                located
                    .relatives
                    .iter()
                    .filter(|(_, p)| hit(p))
                    .map(|(k, _)| *k),
            );
        }
        let mut touched: Vec<u32> = Vec::new();
        // A virtual placement has no position, so only the deletes that name
        // images reach one — and a z-index is not one of those.
        if !matches!(delete.target, Target::Z(_)) && by_name {
            self.virtuals.retain(|&(image, placement), _| {
                let hit = names(image, placement, 0) == Some(true);
                if hit {
                    touched.push(image);
                }
                !hit
            });
        }
        touched.extend(
            anchors
                .iter()
                .filter_map(|anchor| self.placed.remove(anchor))
                .map(|placed| placed.image),
        );
        for key in keys {
            if self.relatives.remove(&key).is_some() {
                touched.push(key.0);
            }
        }
        // A relative placement goes with its parent, and an image left with
        // no placement by that goes too, whatever the case of `d`.
        loop {
            let orphans: Vec<(u32, u32)> = self
                .relatives
                .iter()
                .filter(|(_, relative)| !self.exists(relative.parent))
                .map(|(&key, _)| key)
                .collect();
            if orphans.is_empty() {
                break;
            }
            for key in orphans {
                self.relatives.remove(&key);
                if !self.shown(key.0) {
                    self.graphics.remove(key.0);
                }
            }
        }
        if !delete.free {
            return;
        }
        if let Target::Image { id, .. } = delete.target {
            touched.push(id);
        }
        for image in touched {
            if !self.shown(image) {
                self.graphics.remove(image);
            }
        }
    }
}

/// The anchors in a cell's zerowidth marks, each with the column mark written
/// straight after it.
pub(super) fn anchor_pairs(marks: &[char]) -> impl Iterator<Item = (u32, Option<usize>)> + '_ {
    marks.iter().enumerate().filter_map(|(at, &mark)| {
        let anchor = (mark as u32).wrapping_sub(ANCHOR);
        if anchor >= ANCHOR_MAX {
            return None;
        }
        let column = marks.get(at + 1).and_then(|&next| {
            let index = (next as u32).wrapping_sub(ANCHOR_COLUMN);
            (index < ANCHOR_COLUMN_MAX).then_some(index as usize)
        });
        Some((anchor, column))
    })
}

/// Whether `ch` is one of the private-use codepoints a placement anchors with:
/// either half of the pair.
pub(super) fn is_anchor(ch: char) -> bool {
    let ch = ch as u32;
    (ANCHOR..ANCHOR + ANCHOR_MAX).contains(&ch)
        || (ANCHOR_COLUMN..ANCHOR_COLUMN + ANCHOR_COLUMN_MAX).contains(&ch)
}
