use d3::color::Rgb;

fn rgb(text: &str) -> Option<(f64, f64, f64, f64)> {
    let Rgb { r, g, b, opacity } = text.parse().ok()?;
    Some((r, g, b, opacity))
}

#[test]
fn keywords_and_hex_read_as_d3_color_does() {
    assert_eq!(rgb("steelblue"), Some((70.0, 130.0, 180.0, 1.0)));
    assert_eq!(rgb(" SteelBlue "), Some((70.0, 130.0, 180.0, 1.0)));
    assert_eq!(rgb("#f00"), Some((255.0, 0.0, 0.0, 1.0)));
    assert_eq!(rgb("#ff000080"), Some((255.0, 0.0, 0.0, 128.0 / 255.0)));
    assert!(rgb("#ff00").unwrap().3 == 0.0);
    assert!(rgb("#12345").is_none());
    assert!(rgb("nope").is_none());
    let (r, .., opacity) = rgb("transparent").unwrap();
    assert!(r.is_nan() && opacity == 0.0);
}

#[test]
fn functions_read_integers_percentages_and_hsl() {
    assert_eq!(rgb("rgb(255, 0, 0)"), Some((255.0, 0.0, 0.0, 1.0)));
    assert_eq!(rgb("rgb(100%, 0%, 50%)"), Some((255.0, 0.0, 127.5, 1.0)));
    assert!(rgb("rgb(1.5, 0, 0)").is_none());
    assert_eq!(rgb("rgba(0, 0, 255, 0.5)"), Some((0.0, 0.0, 255.0, 0.5)));
    assert_eq!(rgb("hsl(120, 100%, 50%)"), Some((0.0, 255.0, 0.0, 1.0)));
    assert_eq!(
        rgb("hsla(0, 0%, 100%, .5)"),
        Some((255.0, 255.0, 255.0, 0.5))
    );
}
