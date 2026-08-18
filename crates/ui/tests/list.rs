use gpui::UniformListScrollHandle;

use ui::list::*;

use gpui::{point, px};

#[test]
fn the_bridge_shares_the_lists_scrolling_rather_than_copying_it() {
    // The whole contract, and the one way it can quietly fail: hand the bar
    // a detached clone and it reports on a handle nothing scrolls, which
    // looks exactly like a bar that simply never moves.
    let list = UniformListScrollHandle::new();
    let bridged = scroll_handle(&list);

    bridged.set_offset(point(px(0.0), px(-120.0)));
    assert_eq!(
        list.0.borrow().base_handle.offset().y,
        px(-120.0),
        "the list sees what the bar did"
    );

    list.0
        .borrow()
        .base_handle
        .set_offset(point(px(0.0), px(-40.0)));
    assert_eq!(
        bridged.offset().y,
        px(-40.0),
        "and the bar sees what the list did"
    );
}
