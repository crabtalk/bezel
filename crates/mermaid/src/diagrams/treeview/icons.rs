//! Pele's `diagrams/treeview/icons.ts`.

use super::model::{NodeType, TreeNode};
use crate::{Config, Value, config::Map};

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

/// A file's icon from the configured maps. A file name wins over an
/// extension, and extensions match in lower case with or without the leading
/// dot.
pub fn detect_icon<'c>(name: &str, config: &IconConfig<'c>) -> Option<&'c str> {
    if let Some(by_name) = own(config.filename_icons, name).filter(|icon| !icon.is_empty()) {
        return Some(by_name);
    }
    match name.rfind('.') {
        Some(dot) if dot > 0 => {
            let ext = name[dot..].to_lowercase();
            own(config.extension_icons, &ext).or_else(|| own(config.extension_icons, &ext[1..]))
        }
        _ => None,
    }
}

fn qualify_icon(icon: &str, default_icon_pack: &str) -> String {
    if icon.contains(':') {
        return icon.to_string();
    }
    if glyph(icon).is_some() || default_icon_pack.is_empty() {
        return format!("{BUILTIN_PACK}:{icon}");
    }
    format!("{default_icon_pack}:{icon}")
}

/// The qualified icon name for a node, or `None` when it shows none. An
/// `icon()` annotation always wins; otherwise icons appear only with
/// `showIcons`.
pub fn get_node_icon(node: &TreeNode, config: &IconConfig) -> Option<String> {
    match node.icon.as_deref() {
        Some("none") => return None,
        Some(icon) if !icon.is_empty() => {
            return Some(qualify_icon(icon, config.default_icon_pack));
        }
        _ => {}
    }
    if !config.show_icons {
        return None;
    }
    if node.node_type == NodeType::File {
        match detect_icon(&node.name, config) {
            Some("none") => return None,
            Some(detected) if !detected.is_empty() => {
                return Some(qualify_icon(detected, config.default_icon_pack));
            }
            _ => {}
        }
    }
    Some(format!(
        "{BUILTIN_PACK}:{}",
        if node.node_type == NodeType::Directory {
            "folder"
        } else {
            "file"
        }
    ))
}

pub fn icon_config(config: &Config) -> IconConfig<'_> {
    let c = config.section("treeView");
    let get = |key: &str| c.and_then(|c| c.get(key));
    IconConfig {
        show_icons: get("showIcons").is_some_and(Value::truthy),
        default_icon_pack: get("defaultIconPack").and_then(Value::as_str).unwrap_or(""),
        filename_icons: get("filenameIcons").and_then(Value::as_map),
        extension_icons: get("extensionIcons").and_then(Value::as_map),
    }
}
