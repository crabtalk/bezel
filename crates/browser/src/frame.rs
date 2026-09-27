use crate::host::{Host, Surface};
use gpui::{Bounds, Context, FocusHandle, IntoElement, Pixels, Render, RenderImage, Window};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use std::{
    cell::{Cell, OnceCell, RefCell},
    rc::Rc,
    sync::Arc,
};
use wasm_bindgen::{JsCast, JsValue};
use web_sys::{HtmlCanvasElement, HtmlIFrameElement};

/// A page in an `<iframe>` over the canvas of a web build.
///
/// The iframe is built the first time the frame is painted, beside the
/// window's canvas, and removed when the frame drops. It sits at the element's
/// bounds in every frame the element is painted and is hidden in every frame
/// it is not, or where a [`ui::cover`] recorded after it overlaps it. A hidden
/// iframe stays loaded.
///
/// A site that refuses to be framed (`X-Frame-Options`, CSP
/// `frame-ancestors`) shows the browser's error page. The frame reports
/// nothing about its page and cannot run script in it.
pub struct Frame {
    frame: Rc<Iframe>,
}

impl Frame {
    pub fn new(url: impl Into<String>) -> Self {
        Self {
            frame: Rc::new(Iframe {
                url: RefCell::new(url.into()),
                built: OnceCell::new(),
                placed: Cell::new(None),
                owner: Cell::new(0),
            }),
        }
    }

    /// Before the first paint, replaces the URL the iframe is built with.
    /// After it, navigates the iframe.
    pub fn load(&mut self, url: impl Into<String>) {
        let url = url.into();
        match self.frame.built() {
            Some((_, iframe)) => iframe.set_src(&url),
            None => *self.frame.url.borrow_mut() = url,
        }
    }
}

impl Render for Frame {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        Host {
            surface: self.frame.clone(),
        }
    }
}

struct Iframe {
    /// What the iframe is built with; unread once it is built.
    url: RefCell<String>,
    built: OnceCell<Option<(HtmlCanvasElement, HtmlIFrameElement)>>,
    /// Where the iframe last sat, in page pixels; `None` while hidden.
    placed: Cell<Option<Bounds<f64>>>,
    owner: Cell<u64>,
}

impl Iframe {
    fn built(&self) -> Option<&(HtmlCanvasElement, HtmlIFrameElement)> {
        self.built.get()?.as_ref()
    }

    fn build(&self, window: &Window) -> Option<(HtmlCanvasElement, HtmlIFrameElement)> {
        let handle = HasWindowHandle::window_handle(window).ok()?;
        let RawWindowHandle::WebCanvas(handle) = handle.as_raw() else {
            return None;
        };
        // SAFETY: gpui_web's handle points at the canvas's `JsValue`, which
        // its window holds.
        let canvas = unsafe { handle.obj.cast::<JsValue>().as_ref() };
        let canvas = canvas.clone().dyn_into::<HtmlCanvasElement>().ok()?;
        let parent = canvas.parent_node()?;
        let iframe = canvas
            .owner_document()?
            .create_element("iframe")
            .ok()?
            .dyn_into::<HtmlIFrameElement>()
            .ok()?;
        iframe.set_src(&self.url.borrow());
        let style = iframe.style();
        for (property, value) in [
            ("position", "fixed"),
            ("border", "0"),
            ("margin", "0"),
            ("padding", "0"),
            ("display", "none"),
        ] {
            let _ = style.set_property(property, value);
        }
        parent
            .insert_before(&iframe, canvas.next_sibling().as_ref())
            .ok()?;
        Some((canvas, iframe))
    }
}

impl Surface for Iframe {
    fn focus(&self) -> Option<&FocusHandle> {
        None
    }

    fn owner(&self) -> &Cell<u64> {
        &self.owner
    }

    fn place(&self, bounds: Bounds<Pixels>, window: &Window) {
        let Some((canvas, iframe)) = self.built.get_or_init(|| self.build(window)) else {
            return;
        };
        // gpui's logical pixels are CSS pixels, from the canvas's corner.
        let corner = canvas.get_bounding_client_rect();
        let bounds = Bounds {
            origin: gpui::point(
                corner.left() + f64::from(f32::from(bounds.origin.x)),
                corner.top() + f64::from(f32::from(bounds.origin.y)),
            ),
            size: gpui::size(
                f64::from(f32::from(bounds.size.width)),
                f64::from(f32::from(bounds.size.height)),
            ),
        };
        if self.placed.get() == Some(bounds) {
            return;
        }
        let style = iframe.style();
        for (property, value) in [
            ("left", bounds.origin.x),
            ("top", bounds.origin.y),
            ("width", bounds.size.width),
            ("height", bounds.size.height),
        ] {
            let _ = style.set_property(property, &format!("{value}px"));
        }
        let _ = style.set_property("display", "block");
        self.placed.set(Some(bounds));
    }

    fn park(&self) {
        if let Some((_, iframe)) = self.built()
            && self.placed.take().is_some()
        {
            let _ = iframe.style().set_property("display", "none");
        }
    }

    fn cover(&self) {
        self.park();
    }

    fn still(&self) -> Option<Arc<RenderImage>> {
        None
    }

    fn take_dropped(&self) -> Option<Arc<RenderImage>> {
        None
    }
}

impl Drop for Iframe {
    fn drop(&mut self) {
        if let Some((_, iframe)) = self.built() {
            iframe.remove();
        }
    }
}
