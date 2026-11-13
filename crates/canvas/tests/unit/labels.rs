use crate::{
    path::Rect,
    view::labels::{Key, Layout},
};
use gpui::{Font, px, size};
use markdown::Typography;

#[test]
fn measured_label_overflow_remains_visible_at_viewport_edges() {
    let layout = Layout {
        key: Key {
            text: "a wrapped label".into(),
            typography: Typography::default(),
            font: Font::default(),
            zoom: 1.,
            size: (120., 24.),
            scale: 1.,
        },
        bounds: Rect {
            x: -80.,
            y: -60.,
            w: 160.,
            h: 120.,
        },
    };
    let viewport = size(px(800.), px(600.));
    assert!(layout.visible((-80., 200.), viewport));
    assert!(layout.visible((880., 200.), viewport));
    assert!(layout.visible((200., -60.), viewport));
    assert!(layout.visible((200., 660.), viewport));
    assert!(!layout.visible((-81., 200.), viewport));
    assert!(!layout.visible((881., 200.), viewport));
    assert!(!layout.visible((200., -61.), viewport));
    assert!(!layout.visible((200., 661.), viewport));
}
