//! A document held as markdown and as blocks at once.

use std::{ops::Range, rc::Rc};

use crate::{
    doc::{Block, BlockKind, Doc},
    marks::Marks,
    parse::{parse_at_many, parse_ranges_with, parse_with},
    select::Cursor,
    serialize::{Layout, sentinel, sentinel_index, write_gap_after, write_one, write_one_as},
};

/// A document as markdown and as blocks, each kept up with the other.
///
/// Every block owns a range of the text. The ranges partition it in block
/// order, each running from where the parser starts its block to where it
/// starts the next, and a block that is not written owns an empty one. Read
/// back, the text gives the blocks with their texts normalized.
///
/// [`Self::set_doc`] rewrites the text of the blocks that changed and leaves
/// every other byte as it was. [`Self::set_text`] reads again the blocks whose
/// text changed. Each works on a window of top-level blocks around the change,
/// read back with the top-level block on either side of it and widened until
/// those read as they did, up to the whole document.
pub struct Synced {
    text: String,
    doc: Rc<Doc>,
    /// `doc`'s blocks with their texts normalized: what is written.
    written: Vec<Block>,
    layout: Layout,
    ranges: Vec<Range<usize>>,
    marks: Marks,
    /// Whether the text may define a link reference, which every block reads
    /// wherever it stands, so no window can be read alone.
    references: bool,
}

impl Synced {
    pub fn new(text: impl Into<String>, marks: &Marks) -> Self {
        let text = text.into();
        let parsed = parse_ranges_with(&text, marks);
        let written = parsed.doc.blocks.clone();
        Self {
            layout: Layout::of(&written),
            written,
            doc: Rc::new(parsed.doc),
            ranges: parsed.block_ranges,
            references: defines_references(&text),
            text,
            marks: marks.clone(),
        }
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn doc(&self) -> &Rc<Doc> {
        &self.doc
    }

    /// Each block's range of [`Self::text`], in block order.
    pub fn ranges(&self) -> &[Range<usize>] {
        &self.ranges
    }

    /// The blocks are now `doc`. The blocks that differ from the ones before
    /// are written again.
    pub fn set_doc(&mut self, doc: Rc<Doc>) {
        if Rc::ptr_eq(&self.doc, &doc) {
            return;
        }
        let (old, new) = (&self.doc.blocks, &doc.blocks);
        let (n_old, n_new) = (old.len(), new.len());
        let p = old.iter().zip(new).take_while(|(a, b)| a == b).count();
        if p == n_old && p == n_new {
            self.doc = doc;
            return;
        }
        let room = n_old.min(n_new) - p;
        let q = old
            .iter()
            .rev()
            .zip(new.iter().rev())
            .take(room)
            .take_while(|(a, b)| a == b)
            .count();
        let changed = new[p..n_new - q].iter().cloned().map(|mut block| {
            block.normalize_text();
            block
        });
        self.written.splice(p..n_old - q, changed);
        let layout = self
            .layout
            .respliced(&self.written, p..n_old - q, n_new - q - p);
        let delta = n_new as isize - n_old as isize;
        let old_ix = |ix: usize| (ix as isize - delta) as usize;

        if self.write_changed(&layout, p, n_new - q, delta) {
            self.doc = doc;
            self.layout = layout;
            return;
        }
        let mut s = match p {
            0 => 0,
            _ => Bounds::new(&layout, &self.ranges, &self.text, p, n_new - q, delta)
                .at_or_before(p - 1),
        };
        let mut e = match n_new - q {
            end if end == n_new => n_new,
            end => {
                Bounds::new(&layout, &self.ranges, &self.text, p, n_new - q, delta).at_or_after(end)
            }
        };
        loop {
            let start = snap(&self.text, &self.ranges, s);
            let end = snap(&self.text, &self.ranges, old_ix(e));
            let mut out = String::new();
            let mut chunks = Vec::with_capacity(e - s);
            for ix in s..e {
                let at = out.len();
                write_one(&mut out, &self.written, &layout, ix, &self.marks);
                chunks.push(at..out.len());
            }
            if e == n_new {
                keep_trailing_newlines(&self.text[start..end], &mut out, &mut chunks);
            }
            if s == 0 && e == n_new {
                self.splice(start..end, &out, s..old_ix(e), chunks);
                break;
            }
            let before = match s {
                0 => 0,
                _ => Bounds::new(&layout, &self.ranges, &self.text, p, n_new - q, delta)
                    .at_or_before(s - 1),
            };
            let after =
                Bounds::new(&layout, &self.ranges, &self.text, p, n_new - q, delta).after(e);
            let region = format!(
                "{}{out}{}",
                &self.text[snap(&self.text, &self.ranges, before)..start],
                &self.text[end..snap(&self.text, &self.ranges, old_ix(after))],
            );
            if parse_with(&region, &self.marks)
                == normalized(&self.written[before..after], &self.marks)
            {
                self.splice(start..end, &out, s..old_ix(e), chunks);
                break;
            }
            (s, e) = (before, after);
        }
        self.doc = doc;
        self.layout = layout;
    }

    /// Write blocks `changed` alone, with the gap before them in place of
    /// the one there, and whether that reads back. Nothing is written if not.
    /// `layout` and [`Self::written`] are the new blocks'; the text and the
    /// ranges still the old ones, `delta` blocks fewer after the change.
    fn write_changed(&mut self, layout: &Layout, p: usize, end: usize, delta: isize) -> bool {
        let old_ix = |ix: usize| (ix as isize - delta) as usize;
        let n = self.written.len();
        let previous = (0..p).rev().find(|&ix| layout.at[ix].is_some());
        let start = match previous {
            Some(previous) => match gap_start(&self.text, &self.ranges, p) {
                // The gap ends the block written last, and starts after it.
                Some(start) if start > self.ranges[previous].start => start,
                _ => return false,
            },
            None => 0,
        };
        let stop = snap(&self.text, &self.ranges, old_ix(end));
        let mut out = String::new();
        if let Some(previous) = previous {
            write_gap_after(&mut out, &self.written, layout, previous);
        }
        // Blocks not written between the last one written and the change own
        // empty ranges inside the gap, and move with it.
        let first = previous.map_or(0, |previous| previous + 1);
        let mut chunks = Vec::with_capacity(end - first);
        for ix in first..end {
            let at = out.len();
            write_one(&mut out, &self.written, layout, ix, &self.marks);
            chunks.push(at..out.len());
        }
        if end == n {
            keep_trailing_newlines(&self.text[start..stop], &mut out, &mut chunks);
        }
        let view = Bounds::new(layout, &self.ranges, &self.text, p, end, delta);
        let before = match p {
            0 => 0,
            _ => view.at_or_before(p - 1),
        };
        // The first block written after the change is the one whose reading
        // the new gap can change.
        let after = match (end..n).find(|&ix| layout.at[ix].is_some()) {
            Some(next) => view.after(next),
            None => n,
        };
        let (head, tail) = (
            snap(&self.text, &self.ranges, before),
            snap(&self.text, &self.ranges, old_ix(after)),
        );
        // The gap reached back past the block the region starts at.
        if head > start || stop > tail {
            return false;
        }
        let region = format!("{}{out}{}", &self.text[head..start], &self.text[stop..tail]);
        if parse_with(&region, &self.marks) != normalized(&self.written[before..after], &self.marks)
        {
            return false;
        }
        self.splice(start..stop, &out, first..old_ix(end), chunks);
        true
    }

    /// The text is now `text`. The blocks whose text changed are read again.
    pub fn set_text(&mut self, text: &str) {
        if text == self.text {
            return;
        }
        if self.ranges.is_empty() || self.references || defines_references(text) {
            *self = Self::new(text, &self.marks);
            return;
        }
        let (old, new) = (self.text.as_bytes(), text.as_bytes());
        let mut p = old.iter().zip(new).take_while(|(a, b)| a == b).count();
        while !self.text.is_char_boundary(p) {
            p -= 1;
        }
        let room = old.len().min(new.len()) - p;
        let mut q = old
            .iter()
            .rev()
            .zip(new.iter().rev())
            .take(room)
            .take_while(|(a, b)| a == b)
            .count();
        while !self.text.is_char_boundary(old.len() - q) {
            q -= 1;
        }
        let delta = new.len() as isize - old.len() as isize;
        let shifted = |at: usize| (at as isize + delta) as usize;

        let n = self.ranges.len();
        let holding = |at: usize| {
            self.ranges
                .partition_point(|range| range.start <= at)
                .max(1)
                - 1
        };
        let mut s = self.bounds().at_or_before(holding(p));
        let mut e = self.past_numbering(self.bounds().after(holding(old.len() - q)));
        let (blocks, ranges, start) = loop {
            let before = match s {
                0 => 0,
                _ => self.bounds().at_or_before(s - 1),
            };
            let after = match e {
                e if e == n => n,
                e => self.bounds().after(e),
            };
            let start = snap(&self.text, &self.ranges, before);
            let end = shifted(snap(&self.text, &self.ranges, after));
            let parsed = parse_ranges_with(&text[start..end], &self.marks);
            let (blocks, ranges) = (parsed.doc.blocks, parsed.block_ranges);
            let (lead, trail) = (s - before, after - e);
            let fits = blocks.len() >= lead + trail;
            let leads = fits
                && (0..lead).all(|ix| {
                    same_but_number(&blocks[ix], &self.doc.blocks[before + ix])
                        && start + ranges[ix].start == self.ranges[before + ix].start
                });
            let trails = fits
                && (0..trail).all(|ix| {
                    let at = blocks.len() - trail + ix;
                    same_but_number(&blocks[at], &self.doc.blocks[e + ix])
                        && start + ranges[at].start == shifted(self.ranges[e + ix].start)
                });
            if leads && trails {
                let keep = lead..blocks.len() - trail;
                break (blocks[keep.clone()].to_vec(), ranges[keep].to_vec(), start);
            }
            if before == 0 && after == n {
                *self = Self::new(text, &self.marks);
                return;
            }
            if !leads {
                s = before;
            }
            if !trails {
                e = self.past_numbering(after);
            }
        };

        let ranges = ranges
            .into_iter()
            .map(|range| start + range.start..start + range.end);
        let doc = Rc::make_mut(&mut self.doc);
        doc.blocks.splice(s..e, blocks.iter().cloned());
        doc.renumber();
        self.written.splice(s..e, blocks);
        renumber(&mut self.written);
        let count = ranges.len();
        self.ranges.splice(s..e, ranges);
        for range in &mut self.ranges[s + count..] {
            *range = shifted(range.start)..shifted(range.end);
        }
        self.text = text.to_owned();
        self.close_gaps(s.saturating_sub(1)..s + count + 1);
        self.layout = self.layout.respliced(&self.written, s..e, count);
    }

    /// Where each of `at` lands in the text, `None` for one this cannot place.
    ///
    /// The top-level blocks holding them are written again, with the positions
    /// in, as [`crate::serialize_at_many`] writes a document: a trailing space
    /// with a position after it is no longer trailing, and stays.
    pub fn offsets(&mut self, at: &[Cursor]) -> Vec<Option<usize>> {
        let n = self.doc.blocks.len();
        // Windows of blocks and the positions in them, in block order.
        let mut windows: Vec<(usize, usize, Vec<usize>)> = Vec::new();
        let mut order: Vec<usize> = (0..at.len()).filter(|&ix| at[ix].block < n).collect();
        order.sort_by_key(|&ix| at[ix]);
        for ix in order {
            let s = self.bounds().at_or_before(at[ix].block);
            let e = self.bounds().after(at[ix].block);
            match windows.last_mut() {
                Some(last) if s < last.1 => {
                    last.1 = last.1.max(e);
                    last.2.push(ix);
                }
                _ => windows.push((s, e, vec![ix])),
            }
        }
        // Back to front, so the text a window has yet to rewrite has not
        // moved. Each position is kept against its window's start until the
        // end.
        let mut placed: Vec<(usize, usize)> = Vec::new();
        let mut out = vec![None; at.len()];
        while let Some((mut s, mut e, mut which)) = windows.pop() {
            loop {
                let start = snap(&self.text, &self.ranges, s);
                let end = snap(&self.text, &self.ranges, e);
                let whole = s == 0 && e == n;
                let before = match s {
                    0 => 0,
                    _ => self.bounds().at_or_before(s - 1),
                };
                let after = self.bounds().after(e);
                let expected = normalized(&self.written[before..after], &self.marks);
                let reads = |written: &str| {
                    let region = format!(
                        "{}{written}{}",
                        &self.text[snap(&self.text, &self.ranges, before)..start],
                        &self.text[end..snap(&self.text, &self.ranges, after)],
                    );
                    parse_with(&region, &self.marks) == expected
                };
                let (mut written, mut chunks, mut found) = self.write_at(s..e, at, &which);
                let mut settled = reads(&written);
                if !settled {
                    // The positions may have changed how the text around them
                    // is spelled: write it without them, and place each alone.
                    let (clean, clean_chunks, _) = self.write_at(s..e, at, &[]);
                    if reads(&clean) || whole {
                        found = which
                            .iter()
                            .map(|&ix| {
                                let (one, _, alone) = self.write_at(s..e, at, &[ix]);
                                match alone.first() {
                                    Some(&(_, offset)) if one == clean => (ix, offset),
                                    _ => (ix, clean_chunks[at[ix].block - s].start),
                                }
                            })
                            .collect();
                        (written, chunks, settled) = (clean, clean_chunks, true);
                    }
                }
                if settled {
                    self.splice(start..end, &written, s..e, chunks);
                    placed.retain(|(placed, _)| !which.contains(placed));
                    for &ix in &which {
                        out[ix] = None;
                    }
                    for (ix, offset) in found {
                        placed.push((ix, offset));
                        out[ix] = Some(s);
                    }
                    break;
                }
                (s, e) = (before, after);
                // A wider window takes in every window it now reaches.
                while windows.last().is_some_and(|last| last.1 > s) {
                    let (ws, _, more) = windows.pop().expect("checked");
                    s = s.min(ws);
                    which.extend(more);
                }
                which.extend(
                    placed
                        .iter()
                        .filter(|(ix, _)| out[*ix].is_some_and(|ws| ws < e))
                        .map(|(ix, _)| *ix),
                );
                which.sort_unstable();
                which.dedup();
                for &ix in &which {
                    e = e.max(self.bounds().after(at[ix].block));
                }
            }
        }
        for (ix, offset) in placed {
            out[ix] = out[ix].map(|s| snap(&self.text, &self.ranges, s) + offset);
        }
        out
    }

    /// Blocks `range` written as [`Self::offsets`] writes them, with the
    /// positions `which` of `at` in: the text, each block's range of it, and
    /// where each position landed.
    fn write_at(
        &self,
        range: Range<usize>,
        at: &[Cursor],
        which: &[usize],
    ) -> (String, Vec<Range<usize>>, Vec<(usize, usize)>) {
        let s = range.start;
        let mut texts = self.doc.blocks[range.clone()].to_vec();
        let mut placed = Vec::new();
        let mut order = which.to_vec();
        order.sort_by_key(|&ix| std::cmp::Reverse(at[ix]));
        for ix in order {
            let Some(mark) = sentinel(ix) else { continue };
            let cursor = at[ix];
            let Some(text) = texts[cursor.block - s].text_at_mut(cursor.part) else {
                continue;
            };
            let mut offset = cursor.offset.min(text.text.len());
            // No caret sits inside a mention: its text is its preview's.
            if let Some(mention) = text.mention_around(offset) {
                offset = mention.end;
            }
            if text.text.is_char_boundary(offset) {
                text.insert(offset, mark.encode_utf8(&mut [0; 4]));
                placed.push(ix);
            }
        }
        for block in &mut texts {
            block.normalize_text();
        }
        // The gaps, and which blocks are written at all, are the blocks' own:
        // a position is no reason to write a blank paragraph.
        let window = texts.len();
        let mut blocks = self.written[range.clone()].to_vec();
        let next = (range.end..self.written.len()).find(|&ix| self.layout.at[ix].is_some());
        blocks.extend(next.map(|ix| self.written[ix].clone()));
        let layout = Layout::of(&blocks);

        let mut raw = String::new();
        let mut bounds = Vec::with_capacity(window + 1);
        let mut unwritten = Vec::new();
        for (ix, text) in texts.iter().enumerate() {
            bounds.push(raw.len());
            if layout.at[ix].is_none() {
                // A position in a block that is not written lands where the
                // block would have stood.
                unwritten.extend(
                    which
                        .iter()
                        .filter(|&&placed| at[placed].block == s + ix)
                        .map(|&placed| (placed, raw.len())),
                );
            }
            write_one_as(&mut raw, text, &blocks, &layout, ix, &self.marks);
        }
        bounds.push(raw.len());

        let mut written = String::with_capacity(raw.len());
        let mut clean = vec![0; raw.len() + 1];
        let mut found = Vec::new();
        for (at_raw, c) in raw.char_indices() {
            clean[at_raw] = written.len();
            match sentinel_index(c) {
                Some(ix) if placed.contains(&ix) => found.push((ix, written.len())),
                _ => written.push(c),
            }
        }
        clean[raw.len()] = written.len();
        found.extend(
            unwritten
                .into_iter()
                .map(|(ix, at_raw)| (ix, clean[at_raw])),
        );
        let mut chunks: Vec<Range<usize>> = bounds
            .windows(2)
            .map(|pair| clean[pair[0]]..clean[pair[1]])
            .collect();
        if range.end == self.ranges.len() {
            let start = snap(&self.text, &self.ranges, s);
            keep_trailing_newlines(&self.text[start..], &mut written, &mut chunks);
        }
        (written, chunks, found)
    }

    /// The position each of `offsets` stands for, `None` for one this cannot
    /// place — see [`crate::parse_at_many`].
    pub fn cursors(&self, offsets: &[usize]) -> Vec<Option<Cursor>> {
        let n = self.ranges.len();
        if n == 0 || self.references {
            return parse_at_many(&self.text, offsets, &self.marks).1;
        }
        let start_of = |ix: usize| snap(&self.text, &self.ranges, ix);
        let mut out = vec![None; offsets.len()];
        for (ix, &offset) in offsets.iter().enumerate() {
            let offset = offset.min(self.text.len());
            // A window starts its block at its line where it can, which only
            // ever moves a start back.
            let mut block = self
                .ranges
                .partition_point(|range| range.start <= offset)
                .max(1)
                - 1;
            if block + 1 < n && start_of(block + 1) <= offset {
                block += 1;
            }
            let mut s = self.bounds().at_or_before(block);
            let mut e = self.bounds().after(block);
            out[ix] = loop {
                let (start, end) = (start_of(s), start_of(e));
                let (doc, found) =
                    parse_at_many(&self.text[start..end], &[offset - start], &self.marks);
                let reads = found[0].is_some()
                    && doc.blocks.len() == e - s
                    && doc
                        .blocks
                        .iter()
                        .zip(&self.doc.blocks[s..e])
                        .all(|(a, b)| same_but_number(a, b));
                if reads {
                    break found[0].map(|cursor| Cursor {
                        block: cursor.block + s,
                        ..cursor
                    });
                }
                if s == 0 && e == n {
                    break parse_at_many(&self.text, &[offset], &self.marks).1[0];
                }
                s = match s {
                    0 => 0,
                    _ => self.bounds().at_or_before(s - 1),
                };
                e = match e {
                    e if e == n => n,
                    e => self.bounds().after(e),
                };
            };
        }
        out
    }

    /// Where windows may begin and end over the text as it stands.
    fn bounds(&self) -> Bounds<'_> {
        let n = self.ranges.len();
        Bounds::new(&self.layout, &self.ranges, &self.text, n, n, 0)
    }

    /// The first top-level block at or after `ix` that is not an ordered item.
    /// An ordered item takes its number from the run before it, and a run
    /// starting afresh takes the one it was written with.
    fn past_numbering(&self, mut ix: usize) -> usize {
        while self
            .doc
            .blocks
            .get(ix)
            .is_some_and(|block| matches!(block.kind, BlockKind::Ordered { .. }))
        {
            ix = self.bounds().after(ix);
        }
        ix
    }

    /// Replace `bytes` of the text with `with`, and the ranges of `blocks`
    /// with `chunks`, relative to the start of `bytes`.
    fn splice(
        &mut self,
        bytes: Range<usize>,
        with: &str,
        blocks: Range<usize>,
        chunks: Vec<Range<usize>>,
    ) {
        let start = bytes.start;
        let shift = with.len() as isize - bytes.len() as isize;
        self.text.replace_range(bytes.clone(), with);
        let written = chunks.len();
        self.ranges.splice(
            blocks.clone(),
            chunks
                .into_iter()
                .map(|range| start + range.start..start + range.end),
        );
        let tail = blocks.start + written;
        for range in &mut self.ranges[tail..] {
            *range = (range.start as isize + shift) as usize..(range.end as isize + shift) as usize;
        }
        self.close_gaps(blocks.start.saturating_sub(1)..tail + 1);
        debug_assert!(
            self.ranges
                .windows(2)
                .all(|pair| pair[0].end == pair[1].start)
                && self
                    .ranges
                    .last()
                    .is_none_or(|last| last.end == self.text.len())
                && self.ranges.iter().all(|range| range.start <= range.end),
            "ranges {:?} over {} bytes after {bytes:?} {blocks:?}",
            self.ranges,
            self.text.len()
        );
    }

    /// Hand bytes no block was read from to the block before them, or to the
    /// first block where none is before.
    /// Only the ranges of `blocks` and their neighbours' edges are looked at.
    fn close_gaps(&mut self, blocks: Range<usize>) {
        let len = self.text.len();
        let n = self.ranges.len();
        for ix in blocks.start.max(1)..(blocks.end + 1).min(n) {
            self.ranges[ix - 1].end = self.ranges[ix].start;
        }
        if let Some(first) = self.ranges.first_mut() {
            first.start = 0;
        }
        if let Some(last) = self.ranges.last_mut() {
            last.end = len;
        }
    }
}

/// Where a window starting at block `ix` starts in `text`: the start of its
/// line where only spaces stand before it there, else the block's own start.
/// The text's end for `ix` past the blocks.
fn snap(text: &str, ranges: &[Range<usize>], ix: usize) -> usize {
    let Some(range) = ranges.get(ix) else {
        return text.len();
    };
    let at = range.start;
    let line = text[..at].rfind('\n').map_or(0, |newline| newline + 1);
    let floor = ix.checked_sub(1).map_or(0, |before| ranges[before].start);
    match line >= floor && text[line..at].bytes().all(|b| b == b' ' || b == b'\t') {
        true => line,
        false => at,
    }
}

/// Where a window may begin or end, over a layout of blocks whose bytes are
/// the text's but for blocks `keep..from` of it, which are written afresh in
/// place of the text's and `delta` more in number.
///
/// A block bounds a window when it stands at the top level and its bytes
/// start a line at its first column. Indented bytes were written for what
/// stood before them, and read differently after anything else.
struct Bounds<'a> {
    layout: &'a Layout,
    ranges: &'a [Range<usize>],
    text: &'a str,
    keep: usize,
    from: usize,
    delta: isize,
}

impl<'a> Bounds<'a> {
    fn new(
        layout: &'a Layout,
        ranges: &'a [Range<usize>],
        text: &'a str,
        keep: usize,
        from: usize,
        delta: isize,
    ) -> Self {
        Self {
            layout,
            ranges,
            text,
            keep,
            from,
            delta,
        }
    }

    /// Block `ix`'s number in the text, `None` for one written afresh.
    fn old(&self, ix: usize) -> Option<usize> {
        match ix {
            ix if ix < self.keep => Some(ix),
            ix if ix >= self.from => Some((ix as isize - self.delta) as usize),
            _ => None,
        }
    }

    fn bounds(&self, ix: usize) -> bool {
        self.layout.top(ix)
            && self.old(ix).is_none_or(|old| {
                let at = snap(self.text, self.ranges, old);
                !matches!(self.text.as_bytes().get(at), Some(b' ' | b'\t'))
            })
    }

    /// The bound at or before `ix`, with any block before it that the parser
    /// started where it starts. Such a block is spelled inside the next one's
    /// bytes, and whether it stands depends on what follows it.
    fn at_or_before(&self, ix: usize) -> usize {
        let mut top = (0..=ix.min(self.layout.at.len().saturating_sub(1)))
            .rev()
            .find(|&ix| self.bounds(ix))
            .unwrap_or(0);
        while top > 0 {
            let glued = self
                .old(top)
                .zip(self.old(top - 1))
                .and_then(|(at, before)| Some((self.ranges.get(at)?, self.ranges.get(before)?)))
                .is_some_and(|(at, before)| before.start == at.start);
            if !glued {
                break;
            }
            top -= 1;
        }
        top
    }

    fn at_or_after(&self, ix: usize) -> usize {
        let n = self.layout.at.len();
        (ix..n).find(|&ix| self.bounds(ix)).unwrap_or(n)
    }

    fn after(&self, ix: usize) -> usize {
        self.at_or_after(ix + 1)
    }
}

/// Where the gap before block `ix` starts: the newline that ends the last
/// line holding anything but spaces and quote markers above the block's own.
/// `None` where the block does not start a line.
fn gap_start(text: &str, ranges: &[Range<usize>], ix: usize) -> Option<usize> {
    let mut at = snap(text, ranges, ix);
    if at == 0 || text.as_bytes()[at - 1] != b'\n' {
        return None;
    }
    loop {
        let end = at - 1;
        let line = text[..end].rfind('\n').map_or(0, |newline| newline + 1);
        if line == 0 || !text[line..end].trim_matches([' ', '\t', '>']).is_empty() {
            return Some(end);
        }
        at = line;
    }
}

/// `blocks` as the parser would read them back once written.
fn normalized(blocks: &[Block], marks: &Marks) -> Doc {
    let mut doc = Doc {
        blocks: blocks.to_vec(),
    };
    doc.normalize_with(marks);
    doc
}

/// Carry the newlines `was` ended with onto `out`, the text written for the
/// last blocks, and into the last chunk.
fn keep_trailing_newlines(was: &str, out: &mut String, chunks: &mut [Range<usize>]) {
    let trail = was.len() - was.trim_end_matches('\n').len();
    let have = out.len() - out.trim_end_matches('\n').len();
    for _ in have..trail {
        out.push('\n');
    }
    if let Some(last) = chunks.last_mut() {
        last.end = out.len();
    }
}

fn renumber(blocks: &mut Vec<Block>) {
    let mut doc = Doc {
        blocks: std::mem::take(blocks),
    };
    doc.renumber();
    *blocks = doc.blocks;
}

/// Equal but for an ordered item's number, which the whole document decides.
fn same_but_number(a: &Block, b: &Block) -> bool {
    match (&a.kind, &b.kind) {
        (BlockKind::Ordered { text: x, .. }, BlockKind::Ordered { text: y, .. }) => {
            x == y && a.indent == b.indent && a.quote == b.quote
        }
        _ => a == b,
    }
}

/// Whether `text` may define a link reference, which every block reads
/// wherever it stands.
fn defines_references(text: &str) -> bool {
    text.contains("]:")
        && text.lines().any(|line| {
            let line = line.trim_start_matches(['>', ' ']);
            line.starts_with('[') && line.contains("]:")
        })
}
