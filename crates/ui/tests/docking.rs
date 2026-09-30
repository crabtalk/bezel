use gpui::{Bounds, Point, point, px, size};
use ui::docking::{self, Zone};

#[test]
fn pane_edges_split_and_the_bar_and_centre_join() {
    let bounds = Bounds::new(point(px(100.), px(50.)), size(px(400.), px(300.)));
    for (at, expected) in [
        ((101., 55.), Some(Zone::Join)),
        ((101., 180.), Some(Zone::Left)),
        ((499., 180.), Some(Zone::Right)),
        ((300., 82.), Some(Zone::Top)),
        ((300., 349.), Some(Zone::Bottom)),
        ((300., 200.), Some(Zone::Join)),
        ((99., 200.), None),
    ] {
        assert_eq!(
            docking::zone(bounds, px(24.), point(px(at.0), px(at.1))),
            expected
        );
    }
    assert_eq!(
        docking::zone(Bounds::default(), px(24.), Point::default()),
        None
    );
}

#[test]
fn previews_inset_the_matching_half_or_whole_pane() {
    let bounds = Bounds::new(point(px(100.), px(50.)), size(px(400.), px(300.)));
    for (zone, origin, dimensions) in [
        (Zone::Join, (104., 54.), (392., 292.)),
        (Zone::Left, (104., 54.), (192., 292.)),
        (Zone::Right, (304., 54.), (192., 292.)),
        (Zone::Top, (104., 54.), (392., 142.)),
        (Zone::Bottom, (104., 204.), (392., 142.)),
    ] {
        assert_eq!(
            docking::preview_bounds(bounds, zone),
            Bounds::new(
                point(px(origin.0), px(origin.1)),
                size(px(dimensions.0), px(dimensions.1))
            )
        );
    }
}

#[test]
fn tiny_panes_never_produce_negative_preview_sizes() {
    let bounds = Bounds::new(Point::default(), size(px(3.), px(2.)));
    for zone in [Zone::Join, Zone::Left, Zone::Right, Zone::Top, Zone::Bottom] {
        let preview = docking::preview_bounds(bounds, zone);
        assert!(preview.size.width >= px(0.) && preview.size.height >= px(0.));
        assert!(bounds.contains(&preview.origin));
    }
}
