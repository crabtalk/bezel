use crate::page::{Report, platform::Events};
use anyhow::{Context as _, Result, ensure};
use gpui::{
    DevicePixels, RenderImage, Size, Window,
    external_image::{AtlasTileUpdate, DxgiAdapterId, SharedTexture},
};
use std::{
    os::windows::io::{AsHandle as _, FromRawHandle as _, OwnedHandle},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};
use windows::{
    Foundation::TypedEventHandler,
    Graphics::{
        Capture::{Direct3D11CaptureFramePool, GraphicsCaptureItem, GraphicsCaptureSession},
        DirectX::{Direct3D11::IDirect3DDevice, DirectXPixelFormat},
        SizeInt32,
    },
    UI::Composition::ContainerVisual,
    Win32::{
        Foundation::{HMODULE, LUID},
        Graphics::{
            Direct3D,
            Direct3D11::{
                self, ID3D11Device, ID3D11Device5, ID3D11DeviceContext, ID3D11DeviceContext4,
                ID3D11Fence, ID3D11Texture2D,
            },
            Dxgi::{self, Common, IDXGIAdapter, IDXGIDevice, IDXGIFactory4, IDXGIResource1},
        },
        System::WinRT::Direct3D11::{self as interop, IDirect3DDxgiInterfaceAccess},
    },
    core::{IInspectable, Interface as _},
};

struct Slot {
    texture: ID3D11Texture2D,
    handle: OwnedHandle,
    size: SizeInt32,
    busy: AtomicBool,
}

struct Frame {
    slot: Arc<Slot>,
    fence: Arc<OwnedHandle>,
    value: u64,
}
impl Drop for Frame {
    fn drop(&mut self) {
        self.slot.busy.store(false, Ordering::Release);
    }
}

struct Gpu {
    device: ID3D11Device5,
    context: ID3D11DeviceContext4,
    fence: ID3D11Fence,
    handle: Arc<OwnedHandle>,
    value: u64,
    slots: Vec<Arc<Slot>>,
}

impl Gpu {
    fn new(adapter: DxgiAdapterId) -> Result<(Self, IDirect3DDevice)> {
        unsafe {
            let factory: IDXGIFactory4 = Dxgi::CreateDXGIFactory1()?;
            let adapter: IDXGIAdapter = factory.EnumAdapterByLuid(LUID {
                LowPart: adapter.low,
                HighPart: adapter.high,
            })?;
            let mut device: Option<ID3D11Device> = None;
            let mut context: Option<ID3D11DeviceContext> = None;
            Direct3D11::D3D11CreateDevice(
                &adapter,
                Direct3D::D3D_DRIVER_TYPE_UNKNOWN,
                HMODULE::default(),
                Direct3D11::D3D11_CREATE_DEVICE_BGRA_SUPPORT,
                None,
                Direct3D11::D3D11_SDK_VERSION,
                Some(&mut device),
                None,
                Some(&mut context),
            )?;
            let device: ID3D11Device5 = device.context("D3D11 device unavailable")?.cast()?;
            let context: ID3D11DeviceContext4 =
                context.context("D3D11 context unavailable")?.cast()?;
            let dxgi: IDXGIDevice = device.cast()?;
            let winrt = interop::CreateDirect3D11DeviceFromDXGIDevice(&dxgi)?.cast()?;
            let mut fence: Option<ID3D11Fence> = None;
            device.CreateFence(0, Direct3D11::D3D11_FENCE_FLAG_SHARED, &mut fence)?;
            let fence = fence.context("D3D11 shared fence unavailable")?;
            let handle = fence.CreateSharedHandle(None, 0x10000000, None)?;
            Ok((
                Self {
                    device,
                    context,
                    fence,
                    handle: Arc::new(OwnedHandle::from_raw_handle(handle.0)),
                    value: 0,
                    slots: Vec::new(),
                },
                winrt,
            ))
        }
    }

    fn snapshot(&self, frame: &Frame) -> Result<Arc<RenderImage>> {
        unsafe {
            let mut desc = Direct3D11::D3D11_TEXTURE2D_DESC::default();
            frame.slot.texture.GetDesc(&mut desc);
            desc.Usage = Direct3D11::D3D11_USAGE_STAGING;
            desc.BindFlags = 0;
            desc.MiscFlags = 0;
            desc.CPUAccessFlags = Direct3D11::D3D11_CPU_ACCESS_READ.0 as u32;
            let mut staging = None;
            self.device
                .CreateTexture2D(&desc, None, Some(&mut staging))?;
            let staging: ID3D11Texture2D =
                staging.context("snapshot staging texture unavailable")?;
            self.context.CopyResource(&staging, &frame.slot.texture);
            let mut mapped = Direct3D11::D3D11_MAPPED_SUBRESOURCE::default();
            self.context.Map(
                &staging,
                0,
                Direct3D11::D3D11_MAP_READ,
                0,
                Some(&mut mapped),
            )?;
            let width = desc.Width as usize;
            let height = desc.Height as usize;
            let mut bytes = vec![0; width * height * 4];
            for (row, target) in bytes.chunks_exact_mut(width * 4).enumerate() {
                let source = std::slice::from_raw_parts(
                    (mapped.pData as *const u8).add(row * mapped.RowPitch as usize),
                    width * 4,
                );
                target.copy_from_slice(source);
            }
            self.context.Unmap(&staging, 0);
            let image = image::RgbaImage::from_raw(desc.Width, desc.Height, bytes)
                .context("invalid snapshot dimensions")?;
            Ok(Arc::new(RenderImage::new([image::Frame::new(image)])))
        }
    }

    fn capture(&mut self, source: &ID3D11Texture2D, size: SizeInt32) -> Result<Option<Arc<Frame>>> {
        ensure!(
            (1..=8192).contains(&size.Width) && (1..=8192).contains(&size.Height),
            "capture viewport exceeds 8192 pixels"
        );
        let index = self
            .slots
            .iter()
            .position(|slot| !slot.busy.load(Ordering::Acquire));
        let index = match index {
            Some(index) => index,
            None if self.slots.len() < 3 => self.slots.len(),
            None => return Ok(None),
        };
        if self.slots.get(index).is_none_or(|slot| slot.size != size) {
            let desc = Direct3D11::D3D11_TEXTURE2D_DESC {
                Width: size.Width as u32,
                Height: size.Height as u32,
                MipLevels: 1,
                ArraySize: 1,
                Format: Common::DXGI_FORMAT_B8G8R8A8_UNORM,
                SampleDesc: Common::DXGI_SAMPLE_DESC {
                    Count: 1,
                    Quality: 0,
                },
                Usage: Direct3D11::D3D11_USAGE_DEFAULT,
                BindFlags: 0,
                CPUAccessFlags: 0,
                MiscFlags: (Direct3D11::D3D11_RESOURCE_MISC_SHARED_NTHANDLE
                    | Direct3D11::D3D11_RESOURCE_MISC_SHARED)
                    .0 as u32,
            };
            let mut texture = None;
            unsafe {
                self.device
                    .CreateTexture2D(&desc, None, Some(&mut texture))?;
            }
            let texture: ID3D11Texture2D = texture.context("shared capture texture unavailable")?;
            let resource: IDXGIResource1 = texture.cast()?;
            let handle = unsafe {
                resource.CreateSharedHandle(
                    None,
                    (Dxgi::DXGI_SHARED_RESOURCE_READ | Dxgi::DXGI_SHARED_RESOURCE_WRITE).0,
                    None,
                )?
            };
            let slot = Arc::new(Slot {
                texture,
                handle: unsafe { OwnedHandle::from_raw_handle(handle.0) },
                size,
                busy: AtomicBool::new(false),
            });
            if index == self.slots.len() {
                self.slots.push(slot);
            } else {
                self.slots[index] = slot;
            }
        }
        let mut source_size = Direct3D11::D3D11_TEXTURE2D_DESC::default();
        unsafe {
            source.GetDesc(&mut source_size);
        }
        ensure!(
            source_size.Width == size.Width as u32 && source_size.Height == size.Height as u32,
            "capture content dimensions changed before the frame pool"
        );
        self.value = self
            .value
            .checked_add(1)
            .context("capture fence exhausted")?;
        let slot = self.slots[index].clone();
        // The immediate context is serialized by the capture mutex. Reuse is
        // gated by Frame's last reference, including atlas completion ownership.
        unsafe {
            self.context.CopyResource(&slot.texture, source);
            self.context.Signal(&self.fence, self.value)?;
            self.context.Flush();
        }
        slot.busy.store(true, Ordering::Release);
        Ok(Some(Arc::new(Frame {
            slot,
            fence: self.handle.clone(),
            value: self.value,
        })))
    }
}

pub(super) struct Capture {
    gpu: Arc<Mutex<Gpu>>,
    pool: Direct3D11CaptureFramePool,
    session: GraphicsCaptureSession,
    token: i64,
    latest: Arc<Mutex<Option<Arc<Frame>>>>,
    image: Option<Arc<RenderImage>>,
    atlas: Option<std::sync::Weak<dyn gpui::PlatformAtlas>>,
    shown: Option<Arc<Frame>>,
}

impl Capture {
    pub(super) fn new(
        visual: &ContainerVisual,
        size: SizeInt32,
        adapter: DxgiAdapterId,
        reports: Events,
    ) -> Result<Self> {
        let item = GraphicsCaptureItem::CreateFromVisual(visual)?;
        let (gpu, device) = Gpu::new(adapter)?;
        let gpu = Arc::new(Mutex::new(gpu));
        let latest = Arc::new(Mutex::new(None));
        let output = latest.clone();
        let pool = Direct3D11CaptureFramePool::CreateFreeThreaded(
            &device,
            DirectXPixelFormat::B8G8R8A8UIntNormalized,
            3,
            size,
        )?;
        let producer = gpu.clone();
        let token = pool.FrameArrived(&TypedEventHandler::<
            Direct3D11CaptureFramePool,
            IInspectable,
        >::new(move |pool, _| {
            let result = (|| -> Result<()> {
                let pool = pool.as_ref().context("capture pool closed")?;
                let frame = pool.TryGetNextFrame()?;
                let result = (|| -> Result<()> {
                    let size = frame.ContentSize()?;
                    let access: IDirect3DDxgiInterfaceAccess = frame.Surface()?.cast()?;
                    let texture: ID3D11Texture2D = unsafe { access.GetInterface()? };
                    if let Some(frame) = producer.lock().unwrap().capture(&texture, size)? {
                        let wake = output.lock().unwrap().replace(frame).is_none();
                        if wake {
                            let _ = reports.try_send(Report::Frame);
                        }
                    }
                    Ok(())
                })();
                frame.Close()?;
                result
            })();
            if let Err(error) = result {
                let _ = reports.try_send(Report::Failed(format!("WebView2 capture: {error:#}")));
            }
            Ok(())
        }))?;
        let session = pool.CreateCaptureSession(&item)?;
        session.SetIsCursorCaptureEnabled(false)?;
        session.StartCapture()?;
        Ok(Self {
            gpu,
            pool,
            session,
            token,
            latest,
            image: None,
            atlas: None,
            shown: None,
        })
    }

    pub(super) fn snapshot(
        &self,
        done: crate::page::SnapshotDone,
        executor: &gpui::BackgroundExecutor,
    ) {
        let Some(frame) = self.shown.clone() else {
            done(Err("page has not rendered".into()));
            return;
        };
        let gpu = self.gpu.clone();
        executor
            .spawn(async move {
                let result = gpu
                    .lock()
                    .unwrap()
                    .snapshot(&frame)
                    .map_err(|error| format!("WebView2 snapshot: {error:#}"));
                done(result);
            })
            .detach();
    }

    pub(super) fn paint(
        &mut self,
        bounds: gpui::Bounds<gpui::Pixels>,
        window: &mut Window,
    ) -> Result<()> {
        let atlas = self.atlas.as_ref().and_then(std::sync::Weak::upgrade);
        let changed = atlas
            .as_ref()
            .is_none_or(|atlas| !Arc::ptr_eq(atlas, &window.sprite_atlas));
        let missing = match &self.image {
            Some(image) if !changed => window.insert_blank_image(image, 0)?,
            _ => true,
        };
        let frame = self
            .latest
            .lock()
            .unwrap()
            .take()
            .or_else(|| (changed || missing).then(|| self.shown.clone()).flatten());
        if let Some(frame) = frame {
            let size = Size {
                width: DevicePixels(frame.slot.size.Width),
                height: DevicePixels(frame.slot.size.Height),
            };
            if changed
                || self
                    .image
                    .as_ref()
                    .is_none_or(|image| image.size(0) != size)
            {
                if let (Some(image), Some(atlas)) = (self.image.take(), atlas) {
                    atlas.remove(&gpui::RenderImageParams::from((image.as_ref(), 0)).into());
                }
                self.image = Some(Arc::new(RenderImage::blank(size)?));
                self.atlas = Some(Arc::downgrade(&window.sprite_atlas));
            }
            if let Some(image) = &self.image {
                window.insert_blank_image(image, 0)?;
                let retained = frame.clone();
                window.update_image(
                    image,
                    0,
                    gpui::Bounds::new(gpui::point(DevicePixels(0), DevicePixels(0)), size),
                    AtlasTileUpdate::SharedTexture {
                        texture: SharedTexture {
                            resource: frame.slot.handle.as_handle(),
                            fence: frame.fence.as_handle(),
                            value: frame.value,
                        },
                        origin: gpui::point(DevicePixels(0), DevicePixels(0)),
                    },
                    Some(Box::new(move || drop(retained))),
                )?;
                self.shown = Some(frame);
            }
        }
        if let Some(image) = &self.image {
            window.paint_image(
                bounds,
                bounds,
                gpui::Corners::default(),
                image.clone(),
                0,
                false,
            )?;
        }
        Ok(())
    }
}

impl Drop for Capture {
    fn drop(&mut self) {
        if let Err(error) = self.pool.RemoveFrameArrived(self.token) {
            tracing::debug!(%error, "remove capture callback");
        }
        if let Err(error) = self.session.Close() {
            tracing::debug!(%error, "close browser capture");
        }
        if let Err(error) = self.pool.Close() {
            tracing::debug!(%error, "close capture pool");
        }
        if let (Some(image), Some(atlas)) = (
            &self.image,
            self.atlas.as_ref().and_then(std::sync::Weak::upgrade),
        ) {
            atlas.remove(&gpui::RenderImageParams::from((image.as_ref(), 0)).into());
        }
    }
}
