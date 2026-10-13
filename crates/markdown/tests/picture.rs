//! The rows of a picture's right-click menu, by where the picture is.

use std::path::Path;

use markdown::PictureMenu;
use ui::menu::Item;

fn labels(menu: &PictureMenu) -> Vec<Option<String>> {
    menu.items()
        .iter()
        .map(|item| match item {
            Item::Action { label, .. } => Some(label.to_string()),
            _ => None,
        })
        .collect()
}

#[test]
fn a_picture_on_disk_opens_reveals_and_copies() {
    let menu = PictureMenu::new("shot.png", Some(Path::new("/tmp")));
    let reveal = if cfg!(target_os = "macos") {
        "Reveal in Finder"
    } else {
        "Show in Folder"
    };
    assert_eq!(
        labels(&menu),
        [
            Some("Open".into()),
            Some(reveal.into()),
            None,
            Some("Copy Image".into()),
            Some("Copy Image Address".into()),
        ]
    );
}

#[test]
fn a_file_of_no_picture_format_is_not_copied_as_one() {
    let menu = PictureMenu::new("notes.txt", None);
    assert!(!labels(&menu).contains(&Some("Copy Image".into())));
    assert!(labels(&menu).contains(&Some("Open".into())));
}

#[test]
fn a_picture_on_the_web_only_copies_its_address() {
    let menu = PictureMenu::new("https://example.com/a.png", None);
    assert_eq!(labels(&menu), [Some("Copy Image Address".into())]);
}
