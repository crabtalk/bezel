//! The host's icons.

use super::{Host, decode_entities};

/// The host's markup for icon `name`. The host sees the name as the author
/// meant it, entity codes turned into their characters.
pub fn icon(host: &impl Host, name: &str) -> Option<String> {
    host.icon(&decode_entities(name))
}

/// [`icon`], or `""` when the host has none.
pub fn icon_markup(host: &impl Host, name: &str) -> String {
    icon(host, name).unwrap_or_default()
}

/// Whether the host draws icon `name`.
pub fn has_icon(host: &impl Host, name: &str) -> bool {
    !name.is_empty() && !icon_markup(host, name).is_empty()
}
