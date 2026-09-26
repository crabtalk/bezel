//! Decoded graphics and the clock that animates them.

use super::*;

/// The one repaint an [`Images`] has asked for, shared with the element that
/// paints its snapshot.
#[derive(Clone, Default)]
pub struct Wake(std::rc::Rc<std::cell::Cell<Option<Instant>>>);

impl Wake {
    /// Claim a repaint at `due`. `false` when one is already armed for then or
    /// sooner.
    pub(super) fn arm(&self, due: Instant) -> bool {
        if self.0.get().is_some_and(|armed| armed <= due) {
            return false;
        }
        self.0.set(Some(due));
        true
    }

    /// The repaint armed for `due` has fired.
    pub(super) fn fired(&self, due: Instant) {
        if self.0.get() == Some(due) {
            self.0.set(None);
        }
    }
}

/// Decoded kitty images, held beside the [`Emulator`] whose store they came
/// from, and the clocks of the ones that animate. Keyed by the client's image
/// id, and emptied of whatever the emulator no longer holds.
#[derive(Default)]
pub struct Images {
    decoded: HashMap<u32, Decoded>,
    clocks: HashMap<u32, Clock>,
    wake: Wake,
}

/// One image's frames, each decoded on its own.
pub(super) struct Decoded {
    /// The [`crate::kitty::Image::revision`] this is current to.
    revision: u64,
    /// Each frame with the [`crate::kitty::Image::frame_revisions`] entry it
    /// was decoded at. Empty for an image that did not decode.
    frames: Vec<(u64, Arc<gpui::RenderImage>)>,
}

/// Where an animation's playback has got to.
#[derive(Debug, Clone, Copy)]
pub(super) struct Clock {
    /// The [`crate::kitty::Animation::revision`] playback started at.
    revision: u64,
    frame: usize,
    shown_at: Instant,
    loops: u32,
    /// Held on the last frame of a loading animation, for one to follow.
    waiting: bool,
}

impl Images {
    pub fn new() -> Self {
        Self::default()
    }

    /// The emulator's visible placements, each with its decoded image. Call it
    /// once per frame from the grid hook; images that fail to decode are
    /// dropped rather than painted as a hole.
    pub fn placed(&mut self, emulator: &Emulator) -> Vec<PlacedImage> {
        self.placed_at(emulator, Instant::now())
    }

    /// [`Self::placed`] at `now`, which is what picks each animation's frame.
    pub fn placed_at(&mut self, emulator: &Emulator, now: Instant) -> Vec<PlacedImage> {
        let placements = emulator.placements();
        if placements.is_empty() {
            self.decoded.clear();
            self.clocks.clear();
            return Vec::new();
        }
        let graphics = emulator.graphics();
        self.decoded.retain(|id, _| graphics.get(*id).is_some());
        self.clocks.retain(|id, _| graphics.get(*id).is_some());
        let mut frames: HashMap<u32, (usize, Option<Instant>)> = HashMap::new();
        placements
            .into_iter()
            .filter_map(|placement| {
                let image = graphics.get(placement.image)?;
                let stale = self
                    .decoded
                    .get(&placement.image)
                    .is_none_or(|decoded| decoded.revision != image.revision);
                if stale {
                    let old = self.decoded.remove(&placement.image);
                    self.decoded.insert(placement.image, decode(image, old));
                }
                let (frame_index, next_frame) = *frames
                    .entry(placement.image)
                    .or_insert_with(|| self.frame(placement.image, image, now));
                let decoded = self.decoded.get(&placement.image)?;
                let decoded = decoded.frames.get(frame_index)?.1.clone();
                Some(PlacedImage {
                    row: placement.row,
                    col: placement.col,
                    cols: placement.cols,
                    rows: placement.rows,
                    frame: placement.frame,
                    source: placement.source,
                    z: placement.z,
                    id: placement.image,
                    image: decoded,
                    frame_index,
                    next_frame,
                    wake: self.wake.clone(),
                })
            })
            .collect()
    }

    /// The frame an image shows at `now`, and when the next one is due.
    fn frame(
        &mut self,
        id: u32,
        image: &crate::kitty::Image,
        now: Instant,
    ) -> (usize, Option<Instant>) {
        use crate::kitty::AnimationState;
        let animation = image.animation;
        let count = image.frame_count();
        let start = Clock {
            revision: animation.revision,
            frame: animation.current,
            shown_at: now,
            loops: 0,
            waiting: false,
        };
        let clock = self.clocks.entry(id).or_insert(start);
        if clock.revision != animation.revision {
            *clock = start;
        }
        clock.frame = clock.frame.min(count - 1);
        let gap = |frame: usize| {
            Duration::from_millis(image.gaps.get(frame).copied().unwrap_or(0) as u64)
        };
        if animation.state == AnimationState::Stopped
            || count == 1
            || (0..count).all(|frame| gap(frame).is_zero())
        {
            return (clock.frame, None);
        }
        // A frame arriving while a loading animation waits at the end is
        // picked up where it waited. A clock more than 10,000 frames behind is
        // brought up to now.
        for _ in 0..10_000 {
            let gap = gap(clock.frame);
            let due = clock.shown_at + gap;
            if !gap.is_zero() && now < due {
                return (clock.frame, Some(due));
            }
            let mut next = clock.frame + 1;
            if next == count {
                if animation.state == AnimationState::Loading {
                    clock.waiting = true;
                    return (clock.frame, None);
                }
                if animation.loops != 0 && clock.loops + 1 >= animation.loops {
                    return (clock.frame, None);
                }
                clock.loops += 1;
                next = 0;
            }
            clock.frame = next;
            // A frame that arrived during a wait is shown from now, not from
            // when the wait began.
            clock.shown_at = if std::mem::take(&mut clock.waiting) {
                now
            } else {
                due
            };
        }
        clock.shown_at = now;
        (clock.frame, Some(now))
    }
}

/// An image's frames as gpui paints them, keeping from `old` every frame whose
/// revision has not moved.
pub(super) fn decode(image: &crate::kitty::Image, old: Option<Decoded>) -> Decoded {
    let mut kept: HashMap<u64, Arc<gpui::RenderImage>> = old
        .map(|old| old.frames.into_iter().collect())
        .unwrap_or_default();
    let frames = (0..image.frame_count())
        .map(|index| {
            let revision = image.frame_revisions.get(index).copied().unwrap_or(0);
            let frame = match kept.remove(&revision) {
                Some(frame) => frame,
                None => decode_frame(image, index)?,
            };
            Some((revision, frame))
        })
        .collect::<Option<Vec<_>>>()
        .unwrap_or_default();
    Decoded {
        revision: image.revision,
        frames,
    }
}

/// One frame as the picture gpui paints: BGRA, which is what
/// [`gpui::RenderImage`] holds and what gpui's own decoder converts to.
pub(super) fn decode_frame(
    image: &crate::kitty::Image,
    index: usize,
) -> Option<Arc<gpui::RenderImage>> {
    let mut buffer = match index {
        0 => {
            let first =
                crate::pixels::Rgba::decode(image.format, image.width, image.height, &image.bytes)?;
            image::RgbaImage::from_raw(first.width, first.height, first.bytes)?
        }
        index => {
            image::RgbaImage::from_raw(image.width, image.height, image.frame(index)?.to_vec())?
        }
    };
    for pixel in buffer.as_chunks_mut::<4>().0 {
        pixel.swap(0, 2);
    }
    Some(Arc::new(gpui::RenderImage::new([image::Frame::new(
        buffer,
    )])))
}
