//! What a picture's right-click menu offers, and what each row does.
//!
//! The rows and what each does, not the menu: whoever raises it — on
//! [`crate::Editing::image_menu`], or by hit-testing
//! [`crate::BlockLayouts::picture_bounds`] — holds it open and paints it.

use std::path::{Path, PathBuf};

use gpui::App;

/// A picture as its right-click menu acts on it: its URL as written, and the
/// file that resolves to when it is one.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PictureMenu {
    url: String,
    file: Option<PathBuf>,
}

/// A row of a picture's right-click menu.
#[derive(Clone, Copy)]
enum Action {
    Open,
    Reveal,
    CopyImage,
    CopyAddress,
}

impl PictureMenu {
    /// The picture at `url`, a relative path joined onto `base` as
    /// [`crate::image_source`] joins it. A URL with a scheme names no file.
    pub fn new(url: &str, base: Option<&Path>) -> Self {
        let file = (!url.contains("://")).then(|| match base {
            Some(base) => base.join(url),
            None => PathBuf::from(url),
        });
        Self {
            url: url.to_owned(),
            file,
        }
    }

    /// The rows, `None` for a separator — what [`Self::items`] paints and a
    /// chosen row indexes.
    fn actions(&self) -> Vec<Option<Action>> {
        let mut actions = Vec::new();
        if self.file.is_some() {
            actions.extend([Some(Action::Open), Some(Action::Reveal), None]);
            if self.format().is_some() {
                actions.push(Some(Action::CopyImage));
            }
        }
        actions.push(Some(Action::CopyAddress));
        actions
    }

    pub fn items(&self) -> Vec<ui::menu::Item> {
        self.actions()
            .into_iter()
            .map(|action| match action {
                None => ui::menu::Item::Separator,
                Some(Action::Open) => ui::menu::Item::action("Open"),
                Some(Action::Reveal) => ui::menu::Item::action(if cfg!(target_os = "macos") {
                    "Reveal in Finder"
                } else {
                    "Show in Folder"
                }),
                Some(Action::CopyImage) => ui::menu::Item::action("Copy Image"),
                Some(Action::CopyAddress) => ui::menu::Item::action("Copy Image Address"),
            })
            .collect()
    }

    /// The file's picture format, read off its extension.
    fn format(&self) -> Option<gpui::ImageFormat> {
        let extension = self.file.as_ref()?.extension()?.to_str()?.to_lowercase();
        gpui::ImageFormat::from_mime_type(&format!("image/{extension}"))
    }

    /// Run the row at `row` of [`Self::items`].
    pub fn run(&self, row: usize, cx: &mut App) {
        let Some(Some(action)) = self.actions().get(row).copied() else {
            return;
        };
        match (action, &self.file) {
            (Action::Open, Some(file)) => cx.open_with_system(file),
            (Action::Reveal, Some(file)) => cx.reveal_path(file),
            (Action::CopyImage, Some(file)) => {
                if let (Some(format), Ok(bytes)) = (self.format(), std::fs::read(file)) {
                    cx.write_to_clipboard(gpui::ClipboardItem::new_image(
                        &gpui::Image::from_bytes(format, bytes),
                    ));
                }
            }
            (Action::CopyAddress, _) => {
                cx.write_to_clipboard(gpui::ClipboardItem::new_string(self.url.clone()));
            }
            _ => {}
        }
    }
}
