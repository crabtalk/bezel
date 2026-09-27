//! Animation: frames added, composed and deleted, and playback control.

use super::*;

impl Store {
    /// `a=f`: draw a frame's data over a new frame or an existing one.
    pub(super) fn frame(&mut self, id: u32, command: &Command) -> Result<(), &'static str> {
        use crate::pixels::{Rect, Rgba, draw};
        let data = Rgba::decode(
            command.format,
            command.width,
            command.height,
            &command.payload,
        )
        .ok_or("EINVAL:frame data")?;
        let held: usize = self
            .images
            .values()
            .flat_map(|image| &image.frames)
            .map(Vec::len)
            .sum();
        let image = self.images.get_mut(&id).ok_or("ENOENT:image")?;
        image.ensure_rgba()?;
        if data.width > image.width || data.height > image.height {
            return Err("EINVAL:frame larger than the image");
        }
        let count = image.frame_count();
        let edit = command.rows as usize;
        let mut canvas = if (1..=count).contains(&edit) {
            image.rgba(edit - 1)
        } else if command.columns != 0 {
            let base = command.columns as usize;
            if base > count {
                return Err("EINVAL:no such base frame");
            }
            image.rgba(base - 1)
        } else {
            Rgba::filled(image.width, image.height, command.offset_y)
        };
        if !(1..=count).contains(&edit) && held + canvas.bytes.len() > MAX_FRAME_BYTES {
            return Err("ENOSPC:frames");
        }
        let whole = Rect {
            x: 0,
            y: 0,
            width: data.width,
            height: data.height,
        };
        draw(
            &mut canvas,
            command.x,
            command.y,
            &data,
            whole,
            command.offset_x == 1,
        );
        let gap = match command.z {
            z if z > 0 => Some(z as u32),
            z if z < 0 => Some(0),
            _ => None,
        };
        if (1..=count).contains(&edit) {
            image.set_frame(edit - 1, canvas.bytes);
            if let Some(gap) = gap {
                image.gaps[edit - 1] = gap;
            }
        } else {
            image.frames.push(canvas.bytes);
            image.gaps.push(gap.unwrap_or(DEFAULT_GAP));
            image.stamp(image.frames.len());
        }
        Ok(())
    }

    /// `a=a`.
    pub(super) fn animate(&mut self, command: &Command, id: u32) -> Result<(), &'static str> {
        let image = self.images.get_mut(&id).ok_or("ENOENT:image")?;
        let count = image.frame_count();
        if image.gaps.is_empty() {
            image.gaps.push(0);
        }
        let frame = command.rows as usize;
        if (1..=count).contains(&frame) && command.z != 0 {
            image.gaps[frame - 1] = command.z.max(0) as u32;
        }
        let animation = &mut image.animation;
        let current = command.columns as usize;
        if (1..=count).contains(&current) {
            animation.current = current - 1;
            animation.revision += 1;
        }
        // `s` and `v` arrive in the fields `a=t` reads as a size.
        let state = match command.width {
            1 => Some(AnimationState::Stopped),
            2 => Some(AnimationState::Loading),
            3 => Some(AnimationState::Running),
            _ => None,
        };
        if let Some(state) = state {
            animation.state = state;
            animation.revision += 1;
        }
        if command.height != 0 {
            animation.loops = command.height - 1;
            animation.revision += 1;
        }
        Ok(())
    }

    /// `a=c`: frame `r`'s pixels in a rectangle onto frame `c`'s.
    pub(super) fn compose(&mut self, command: &Command, id: u32) -> Result<(), &'static str> {
        use crate::pixels::{Rect, draw};
        let image = self.images.get_mut(&id).ok_or("ENOENT:image")?;
        let count = image.frame_count();
        let (from, onto) = (command.rows as usize, command.columns as usize);
        if !(1..=count).contains(&from) || !(1..=count).contains(&onto) {
            return Err("ENOENT:frame");
        }
        image.ensure_rgba()?;
        let size = |value: u32, whole: u32| if value == 0 { whole } else { value };
        let source = Rect {
            x: command.offset_x,
            y: command.offset_y,
            width: size(command.source_width, image.width),
            height: size(command.source_height, image.height),
        };
        let target = Rect {
            x: command.x,
            y: command.y,
            ..source
        };
        if !source.within(image.width, image.height) || !target.within(image.width, image.height) {
            return Err("EINVAL:rectangle out of bounds");
        }
        if from == onto && source.overlaps(&target) {
            return Err("EINVAL:rectangles overlap");
        }
        let over = image.rgba(from - 1);
        let mut canvas = image.rgba(onto - 1);
        let replace = command.cursor_movement == CursorMovement::None;
        draw(&mut canvas, target.x, target.y, &over, source, replace);
        image.set_frame(onto - 1, canvas.bytes);
        Ok(())
    }

    /// `d=f`: delete frame `r`, the first when it is zero. `d=F` on an image
    /// with one frame frees the image.
    pub(super) fn delete_frame(&mut self, command: &Command, id: u32) -> Result<(), &'static str> {
        let image = self.images.get_mut(&id).ok_or("ENOENT:image")?;
        if image.frames.is_empty() {
            if command.delete == 'F' {
                self.remove(id);
            }
            return Ok(());
        }
        let frame = (command.rows as usize).clamp(1, image.frame_count()) - 1;
        if frame == 0 {
            image.bytes = image.frames.remove(0);
        } else {
            image.frames.remove(frame - 1);
        }
        if frame < image.gaps.len() {
            image.gaps.remove(frame);
        }
        if frame < image.frame_revisions.len() {
            image.frame_revisions.remove(frame);
        }
        let animation = &mut image.animation;
        if animation.current > image.frames.len() {
            animation.current = image.frames.len();
        } else if frame < animation.current {
            animation.current -= 1;
        }
        animation.revision += 1;
        image.revision += 1;
        Ok(())
    }
}

impl Image {
    /// How many frames it has, the first included.
    pub fn frame_count(&self) -> usize {
        1 + self.frames.len()
    }

    /// Frame `index`'s bytes, 0-based.
    pub fn frame(&self, index: usize) -> Option<&[u8]> {
        match index {
            0 => Some(&self.bytes),
            index => self.frames.get(index - 1).map(Vec::as_slice),
        }
    }

    /// Decode the first frame into RGBA, once, and give it its gap.
    pub(super) fn ensure_rgba(&mut self) -> Result<(), &'static str> {
        if self.gaps.is_empty() {
            self.gaps.push(0);
        }
        if self.format == Format::Rgba && !self.frames.is_empty() {
            return Ok(());
        }
        let rgba = crate::pixels::Rgba::decode(self.format, self.width, self.height, &self.bytes)
            .ok_or("EINVAL:image data")?;
        self.format = Format::Rgba;
        self.width = rgba.width;
        self.height = rgba.height;
        self.bytes = rgba.bytes;
        Ok(())
    }

    /// Frame `index` as a buffer to draw on. Only called once
    /// [`Self::ensure_rgba`] has made every frame RGBA.
    pub(super) fn rgba(&self, index: usize) -> crate::pixels::Rgba {
        crate::pixels::Rgba {
            width: self.width,
            height: self.height,
            bytes: self.frame(index).unwrap_or_default().to_vec(),
            opaque: false,
        }
    }

    pub(super) fn set_frame(&mut self, index: usize, bytes: Vec<u8>) {
        match index {
            0 => self.bytes = bytes,
            index => self.frames[index - 1] = bytes,
        }
        self.stamp(index);
    }

    /// Frame `index`'s pixels changed.
    pub(super) fn stamp(&mut self, index: usize) {
        self.revision += 1;
        if self.frame_revisions.len() <= index {
            self.frame_revisions.resize(index + 1, 0);
        }
        self.frame_revisions[index] = self.revision;
    }
}
