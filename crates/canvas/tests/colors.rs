use canvas::model::{self, Node, TextMode};
use theme::Theme;

#[test]
fn label_color_resolves_presets_and_hex_and_resets_to_default() {
    for theme in [Theme::dark(), Theme::light()] {
        let mut node = Node {
            kind: model::TEXT.into(),
            text_mode: Some(TextMode::Label),
            color: Some("1".into()),
            ..Default::default()
        };
        assert_eq!(node.text_color(&theme), Some(theme.danger));
        node.color = Some("#ff0000".into());
        assert_eq!(node.text_color(&theme), Some(gpui::rgb(0xff0000).into()));
        node.color = None;
        assert_eq!(node.text_color(&theme), None);
    }
}

#[test]
fn document_and_image_colors_do_not_override_their_content() {
    let theme = Theme::dark();
    let mut node = Node {
        kind: model::TEXT.into(),
        color: Some("1".into()),
        ..Default::default()
    };
    assert_eq!(node.text_color(&theme), None);
    node.kind = model::FILE.into();
    node.text_mode = Some(TextMode::Label);
    assert_eq!(node.text_color(&theme), None);
}
