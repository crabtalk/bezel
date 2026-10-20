//! Pele's `diagrams/treeview/icons.ts`.

use super::TreeViewConfig;
use super::model::{NodeType, TreeNode};
use crate::js::JsValue;
use crate::{Value, config::Map};

/// Mermaid's name for its built-in pack. Only `file` and `folder` are built
/// in; every other icon name is offered to the host's icon resolver.
pub const BUILTIN_PACK: &str = "mermaid-treeview";

/// Outlines on a 24 by 24 grid.
pub const GLYPHS: [(&str, &str); 2] = [
    (
        "folder",
        "M3 6.5A1.5 1.5 0 0 1 4.5 5h4.6l2.4 2.5h8A1.5 1.5 0 0 1 21 9v9a1.5 1.5 0 0 1-1.5 1.5h-15A1.5 1.5 0 0 1 3 18z",
    ),
    (
        "file",
        "M6.5 3h7L19 8.5v11a1.5 1.5 0 0 1-1.5 1.5h-11A1.5 1.5 0 0 1 5 19.5v-15A1.5 1.5 0 0 1 6.5 3zM13.5 3v5.5H19",
    ),
];

pub fn glyph(name: &str) -> Option<&'static str> {
    GLYPHS
        .iter()
        .find(|(key, _)| *key == name)
        .map(|(_, path)| *path)
}

pub struct IconConfig<'c> {
    pub show_icons: bool,
    pub default_icon_pack: &'c str,
    pub filename_icons: Option<&'c Map>,
    pub extension_icons: Option<&'c Map>,
}

fn own<'c>(map: Option<&'c Map>, key: &str) -> Option<&'c str> {
    map?.get(key)?.as_str()
}

impl<'c> IconConfig<'c> {
    pub fn new(config: &'c TreeViewConfig) -> Self {
        Self {
            show_icons: config.show_icons.as_ref().is_some_and(Value::truthy),
            default_icon_pack: config.default_icon_pack.as_deref().unwrap_or(""),
            filename_icons: config.filename_icons.as_ref(),
            extension_icons: config.extension_icons.as_ref(),
        }
    }

    /// A file's icon from the configured maps. A file name wins over an
    /// extension, and extensions match in lower case with or without the
    /// leading dot.
    fn detect(&self, name: &str) -> Option<&'c str> {
        if let Some(by_name) = own(self.filename_icons, name).filter(|icon| !icon.is_empty()) {
            return Some(by_name);
        }
        match name.rfind('.') {
            Some(dot) if dot > 0 => {
                let ext = name[dot..].to_lowercase();
                own(self.extension_icons, &ext).or_else(|| own(self.extension_icons, &ext[1..]))
            }
            _ => None,
        }
    }

    fn qualify(&self, icon: &str) -> String {
        if icon.contains(':') {
            return icon.to_string();
        }
        if glyph(icon).is_some() || self.default_icon_pack.is_empty() {
            return format!("{BUILTIN_PACK}:{icon}");
        }
        format!("{}:{icon}", self.default_icon_pack)
    }

    /// The qualified icon name for a node, or `None` when it shows none. An
    /// `icon()` annotation always wins; otherwise icons appear only with
    /// `showIcons`.
    pub fn of(&self, node: &TreeNode) -> Option<String> {
        match node.icon.as_deref() {
            Some("none") => return None,
            Some(icon) if !icon.is_empty() => return Some(self.qualify(icon)),
            _ => {}
        }
        if !self.show_icons {
            return None;
        }
        if node.node_type == NodeType::File {
            match self.detect(&node.name) {
                Some("none") => return None,
                Some(detected) if !detected.is_empty() => return Some(self.qualify(detected)),
                _ => {}
            }
        }
        Some(format!(
            "{BUILTIN_PACK}:{}",
            match node.node_type {
                NodeType::Directory => "folder",
                NodeType::File => "file",
            }
        ))
    }
}
