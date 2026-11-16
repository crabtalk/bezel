use crate::page::platform::transport::{Outbox, Packet};
use anyhow::{Context as _, Result, ensure};
use gpui::{
    Bounds, DevicePixels, RenderImage, RenderImageParams, Window,
    external_image::{self, AtlasTileUpdate},
    native_view::Viewport,
    point, size,
};
use serde_json::{Value, json};
use std::{
    collections::HashMap,
    fs::File,
    os::{
        fd::{AsFd as _, AsRawFd as _, OwnedFd},
        unix::fs::FileExt as _,
    },
    sync::{
        Arc, Mutex, Weak,
        atomic::{AtomicBool, Ordering},
    },
};

const MAX_DIMENSION: u32 = 8192;
const ARGB8888: u32 = u32::from_le_bytes(*b"AR24");

#[derive(Clone, Copy, PartialEq)]
pub(super) struct Configuration {
    pub(super) viewport: Viewport,
    pub(super) scale: f32,
    pub(super) generation: u32,
}

impl Configuration {
    pub(super) fn command(self) -> Value {
        json!({"cmd":"resize", "width": f32::from(self.viewport.width).ceil() as u32,
            "height":f32::from(self.viewport.height).ceil() as u32, "scale":self.scale, "generation":self.generation})
    }
}

struct Buffer {
    descriptor: OwnedFd,
    width: u32,
    height: u32,
    stride: u32,
    offset: u32,
    modifier: u64,
    hardware: bool,
}
impl Buffer {
    fn parse(info: &Value, descriptor: OwnedFd) -> Result<(u32, Self)> {
        let integer = |key: &str| -> Result<u32> {
            u32::try_from(
                info[key]
                    .as_u64()
                    .with_context(|| format!("Missing browser buffer {key}"))?,
            )
            .context("Browser buffer field exceeds u32")
        };
        let width = integer("width")?;
        let height = integer("height")?;
        let stride = integer("stride")?;
        let offset = integer("offset")?;
        ensure!(
            (1..=MAX_DIMENSION).contains(&width) && (1..=MAX_DIMENSION).contains(&height),
            "Invalid browser buffer dimensions"
        );
        ensure!(
            stride >= width * 4 && stride <= MAX_DIMENSION * 16,
            "Invalid browser buffer stride"
        );
        ensure!(
            integer("fourcc")? == ARGB8888,
            "Unsupported browser pixel format"
        );
        let hardware = match info["kind"].as_str() {
            Some("dmabuf") => true,
            Some("shm") => false,
            _ => anyhow::bail!("Unknown browser buffer kind"),
        };
        let modifier = if hardware {
            info["modifier"]
                .as_u64()
                .or_else(|| info["modifier"].as_i64().map(|value| value as u64))
                .context("Missing DMA-BUF modifier")?
        } else {
            0
        };
        Ok((
            integer("buffer")?,
            Self {
                descriptor,
                width,
                height,
                stride,
                offset,
                modifier,
                hardware,
            },
        ))
    }
    fn size(&self) -> gpui::Size<DevicePixels> {
        size(
            DevicePixels(self.width as i32),
            DevicePixels(self.height as i32),
        )
    }
    fn pixels(&self) -> Result<Vec<u8>> {
        ensure!(!self.hardware, "DMA-BUF pixels require WPE readback");
        // SAFETY: fcntl only inspects the owned descriptor.
        let seals = unsafe { libc::fcntl(self.descriptor.as_raw_fd(), libc::F_GET_SEALS) };
        ensure!(
            seals >= 0
                && seals & (libc::F_SEAL_SHRINK | libc::F_SEAL_WRITE)
                    == (libc::F_SEAL_SHRINK | libc::F_SEAL_WRITE),
            "Browser pixel file is not immutable"
        );
        let file = File::from(self.descriptor.try_clone()?);
        let row = self.width as usize * 4;
        let mut pixels = vec![0; row * self.height as usize];
        for (index, target) in pixels.chunks_exact_mut(row).enumerate() {
            file.read_exact_at(
                target,
                self.offset as u64 + index as u64 * self.stride as u64,
            )?;
        }
        Ok(pixels)
    }
}

struct Lease {
    outbox: Outbox,
    page: u32,
    id: u32,
    rendered: AtomicBool,
}
impl Lease {
    fn presented(&self) {
        if !self.rendered.swap(true, Ordering::AcqRel) {
            let _ = self
                .outbox
                .send(self.page, json!({"cmd":"presented", "buffer":self.id}));
        }
    }
}
impl Drop for Lease {
    fn drop(&mut self) {
        self.presented();
        let _ = self
            .outbox
            .send(self.page, json!({"cmd":"released", "buffer":self.id}));
    }
}
struct Frame {
    buffer: Arc<Buffer>,
    lease: Arc<Lease>,
    generation: u32,
}
impl Frame {
    fn upload(
        self: &Arc<Self>,
        image: &RenderImage,
        window: &mut Window,
        previous: Option<Arc<Self>>,
    ) -> Result<()> {
        let buffer = &self.buffer;
        let import = if buffer.hardware {
            Some(window.sprite_atlas.import_dmabuf(&external_image::Dmabuf {
                fd: buffer.descriptor.as_fd(),
                width: buffer.width,
                height: buffer.height,
                fourcc: ARGB8888,
                modifier: buffer.modifier,
                offset: buffer.offset,
                stride: buffer.stride,
            })?)
        } else {
            None
        };
        let pixels = if buffer.hardware {
            Vec::new()
        } else {
            buffer.pixels()?
        };
        let update = match &import {
            Some(import) => AtlasTileUpdate::Dmabuf {
                buffer: import.id,
                origin: point(DevicePixels(0), DevicePixels(0)),
            },
            None => AtlasTileUpdate::Pixels {
                bytes: &pixels,
                bytes_per_row: buffer.width * 4,
            },
        };
        let frame = self.clone();
        let complete = Box::new(move || {
            frame.lease.presented();
            drop(previous);
            drop(import);
        });
        ensure!(
            window.update_image(
                image,
                0,
                Bounds::new(point(DevicePixels(0), DevicePixels(0)), buffer.size()),
                update,
                Some(complete)
            )?,
            "GPUI cannot update the page image"
        );
        Ok(())
    }
}

#[derive(Default)]
pub(super) struct Surface {
    buffers: HashMap<u32, Arc<Buffer>>,
    leases: HashMap<u32, Weak<Lease>>,
    pending: Option<Arc<Frame>>,
    pub(super) cursor: Option<String>,
}
impl Surface {
    pub(super) fn clear(&mut self) {
        *self = Self::default();
    }
}

pub(super) enum Received {
    Unchanged,
    Changed,
    Snapshot {
        request: Option<u64>,
        image: Arc<RenderImage>,
    },
}
impl Surface {
    pub(super) fn receive(
        &mut self,
        configuration: Option<Configuration>,
        outbox: &Outbox,
        packet: Packet,
    ) -> Result<Received> {
        let Packet {
            kind,
            page,
            bytes,
            mut descriptor,
        } = packet;
        if kind == b'K' {
            self.cursor = Some(String::from_utf8(bytes.to_vec())?);
            return Ok(Received::Changed);
        }
        let info: Value = serde_json::from_slice(&bytes)?;
        match kind {
            b'B' => {
                let (id, buffer) = Buffer::parse(
                    &info,
                    descriptor.take().context("Missing browser descriptor")?,
                )?;
                let surface = &mut *self;
                ensure!(
                    surface.buffers.len() < 64,
                    "Too many unclaimed browser buffers"
                );
                surface.buffers.insert(id, Arc::new(buffer));
            }
            b'F' => {
                let id = u32::try_from(
                    info["buffer"]
                        .as_u64()
                        .context("Missing browser frame id")?,
                )?;
                let generation = u32::try_from(
                    info["generation"]
                        .as_u64()
                        .context("Missing viewport generation")?,
                )?;
                let surface = &mut *self;
                let buffer = surface
                    .buffers
                    .remove(&id)
                    .context("Browser frame has no buffer")?;
                surface.leases.retain(|_, lease| lease.strong_count() > 0);
                let lease = surface
                    .leases
                    .get(&id)
                    .and_then(Weak::upgrade)
                    .unwrap_or_else(|| {
                        let lease = Arc::new(Lease {
                            outbox: outbox.clone(),
                            page,
                            id,
                            rendered: AtomicBool::new(false),
                        });
                        surface.leases.insert(id, Arc::downgrade(&lease));
                        lease
                    });
                let frame = Arc::new(Frame {
                    buffer,
                    lease,
                    generation,
                });
                if configuration.is_some_and(|config| {
                    config.generation == generation
                        && (f32::from(config.viewport.width).ceil() * config.scale
                            - frame.buffer.width as f32)
                            .abs()
                            <= 1.0
                        && (f32::from(config.viewport.height).ceil() * config.scale
                            - frame.buffer.height as f32)
                            .abs()
                            <= 1.0
                }) {
                    surface.pending = Some(frame);
                    return Ok(Received::Changed);
                }
            }
            b'X' => {
                if let Some(id) = info["buffer"].as_u64() {
                    self.buffers.remove(&(id as u32));
                }
            }
            b'P' => {
                let (_, buffer) = Buffer::parse(
                    &info,
                    descriptor.take().context("Missing browser descriptor")?,
                )?;
                let pixels = buffer.pixels()?;
                let image = image::RgbaImage::from_raw(buffer.width, buffer.height, pixels)
                    .context("Invalid snapshot pixels")?;
                return Ok(Received::Snapshot {
                    request: info["request"].as_u64(),
                    image: Arc::new(RenderImage::new([image::Frame::new(image)])),
                });
            }
            _ => anyhow::bail!("Unknown browser presentation event"),
        }
        Ok(Received::Unchanged)
    }
}

pub(super) struct Presenter {
    outbox: Outbox,
    page: u32,
    current: Option<Arc<Frame>>,
    waiting: Option<Arc<Frame>>,
    atlases: Vec<(Weak<dyn gpui::PlatformAtlas>, gpui::AtlasKey)>,
    focus: Option<(bool, bool)>,
}
impl Presenter {
    pub(super) fn new(outbox: Outbox, page: u32) -> Self {
        Self {
            outbox,
            page,
            current: None,
            waiting: None,
            atlases: Vec::new(),
            focus: None,
        }
    }
    pub(super) fn sync_focus(&mut self, focused: bool, active: bool) {
        if self.focus != Some((focused, active)) {
            let _ = self
                .outbox
                .send(self.page, json!({"cmd":"focus","value":u8::from(focused)}));
            let _ = self
                .outbox
                .send(self.page, json!({"cmd":"active","value":u8::from(active)}));
            self.focus = Some((focused, active));
        }
    }
    pub(super) fn snapshot_frame(&self) -> Option<(u32, u32)> {
        self.current
            .as_ref()
            .map(|frame| (frame.lease.id, frame.generation))
    }
    fn retire(&mut self) {
        for (atlas, key) in self.atlases.drain(..) {
            if let Some(atlas) = atlas.upgrade() {
                atlas.remove(&key);
            }
        }
    }
    pub(super) fn paint(
        &mut self,
        surface: &Mutex<Surface>,
        configuration: Option<Configuration>,
        image: &mut Option<Arc<RenderImage>>,
        bounds: Bounds<gpui::Pixels>,
        window: &mut Window,
    ) -> Result<()> {
        let next = surface.lock().unwrap().pending.take();
        let frame = next.as_ref().or(self.current.as_ref()).cloned();
        let Some(frame) = frame else {
            return Ok(());
        };
        if !configuration.is_some_and(|config| config.generation == frame.generation) {
            return Ok(());
        }
        let result = (|| -> Result<()> {
            if image
                .as_ref()
                .is_none_or(|image| image.size(0) != frame.buffer.size())
            {
                self.retire();
                *image = Some(Arc::new(RenderImage::blank(frame.buffer.size())?));
            }
            let target = image.as_ref().unwrap();
            let key: gpui::AtlasKey = RenderImageParams::from((&**target, 0)).into();
            if !self.atlases.iter().any(|(atlas, stored)| {
                *stored == key
                    && atlas
                        .upgrade()
                        .is_some_and(|atlas| Arc::ptr_eq(&atlas, &window.sprite_atlas))
            }) {
                self.atlases
                    .push((Arc::downgrade(&window.sprite_atlas), key));
            }
            let empty = window.insert_blank_image(target, 0)?;
            if next.is_some() || empty {
                match frame.upload(target, window, self.current.clone()) {
                    Ok(()) => {
                        self.current = Some(frame.clone());
                        self.waiting.take();
                    }
                    Err(error) if frame.buffer.hardware => {
                        let _ = self
                            .outbox
                            .send(self.page, json!({"cmd":"readback","buffer":frame.lease.id}));
                        self.waiting = Some(frame.clone());
                        tracing::debug!("WPE frame requires pixel fallback: {error:#}");
                    }
                    Err(error) => return Err(error),
                }
            }
            if self
                .current
                .as_ref()
                .is_some_and(|shown| shown.generation == frame.generation)
            {
                window.paint_image(bounds, bounds, Default::default(), target.clone(), 0, false)?;
            }
            Ok(())
        })();
        result
    }
}
impl Drop for Presenter {
    fn drop(&mut self) {
        self.retire();
    }
}
