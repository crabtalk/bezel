//! Where the gallery puts a picture that arrives with no address.
//!
//! A screenshot off the clipboard is bytes and nothing else, so somebody has to
//! choose a place before a document can point at one. That choice is the app's:
//! a real one writes into whatever it saves alongside the document. The gallery
//! saves nothing, so it writes beside the temporary files and hands back the
//! path — enough to paint, and gone with the next reboot.
//!
//! Install with `editor::set_image_store(cx, store::of)`.

use editor::Source;

/// A dropped file stays where it is. Only the bytes need somewhere to go, and
/// on the web there is nowhere at all — the browser build takes the paste and
/// has nothing to answer with.
pub fn of(source: Source) -> Option<String> {
    match source {
        Source::File(path) => Some(path.to_string_lossy().into_owned()),
        Source::Bytes(image) => write(image),
    }
}

#[cfg(target_arch = "wasm32")]
fn write(_: &gpui::Image) -> Option<String> {
    None
}

#[cfg(not(target_arch = "wasm32"))]
fn write(image: &gpui::Image) -> Option<String> {
    let path =
        std::env::temp_dir().join(format!("bezel-{}.{}", image.id, image.format.extension()));
    std::fs::write(&path, &image.bytes).ok()?;
    Some(path.to_string_lossy().into_owned())
}
