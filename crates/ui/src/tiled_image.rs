use gpui::{
    AnyElement, App, Bounds, ContentMask, Corners, Element, ElementId, GlobalElementId,
    ImageSource, ImgResourceLoader, InspectorElementId, IntoElement, LayoutId, Pixels, Resource,
    Size, Style, Window, img, point, prelude::*, px, relative,
};

pub struct TiledImage {
    source: Resource,
    scale: f32,
}

impl TiledImage {
    pub fn new(source: impl Into<Resource>) -> Self {
        Self {
            source: source.into(),
            scale: 1.,
        }
    }

    pub fn scale(mut self, scale: f32) -> Self {
        self.scale = scale;
        self
    }
}

impl IntoElement for TiledImage {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}

impl Element for TiledImage {
    type RequestLayoutState = (AnyElement, LayoutId);
    type PrepaintState = Size<Pixels>;

    fn id(&self) -> Option<ElementId> {
        None
    }
    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let mut intrinsic = img(ImageSource::Resource(self.source.clone()))
            .absolute()
            .flex_none()
            .into_any_element();
        let measured = intrinsic.request_layout(window, cx);
        let mut style = Style::default();
        style.size.width = relative(1.).into();
        style.size.height = relative(1.).into();
        (
            window.request_layout(style, [measured], cx),
            (intrinsic, measured),
        )
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        state: &mut Self::RequestLayoutState,
        window: &mut Window,
        _: &mut App,
    ) -> Self::PrepaintState {
        window
            .layout_bounds(state.1)
            .size
            .map(|length| length * self.scale)
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut Self::RequestLayoutState,
        tile: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        let (width, height) = (f32::from(tile.width), f32::from(tile.height));
        if !width.is_finite() || !height.is_finite() || width <= 0. || height <= 0. {
            return;
        }
        let visible = bounds.intersect(&window.content_mask().bounds);
        if visible.size.width <= px(0.) || visible.size.height <= px(0.) {
            return;
        }
        let Some(Ok(image)) = window.use_asset::<ImgResourceLoader>(&self.source, cx) else {
            return;
        };
        let first_x = (f32::from(visible.left() - bounds.left()) / width).floor() as i64;
        let first_y = (f32::from(visible.top() - bounds.top()) / height).floor() as i64;
        let last_x = (f32::from(visible.right() - bounds.left()) / width).ceil() as i64;
        let last_y = (f32::from(visible.bottom() - bounds.top()) / height).ceil() as i64;
        window.with_content_mask(
            Some(ContentMask {
                bounds: visible,
                ..Default::default()
            }),
            |window| {
                for y in first_y..last_y {
                    for x in first_x..last_x {
                        let origin =
                            bounds.origin + point(px(x as f32 * width), px(y as f32 * height));
                        let bounds = Bounds {
                            origin,
                            size: *tile,
                        };
                        let _ = window.paint_image(
                            bounds,
                            bounds,
                            Corners::default(),
                            image.clone(),
                            0,
                            false,
                        );
                    }
                }
            },
        );
    }
}
