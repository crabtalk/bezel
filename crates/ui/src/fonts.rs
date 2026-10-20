//! The fonts an app paints with, for gpui's text system and for SVG laid out
//! and drawn with usvg.
//!
//! Natively, [`database`] is the system's fonts. In a browser it is the faces
//! given to [`add`], and a face given to `cx.text_system().add_fonts` directly
//! paints text and is missing from SVG.

use std::{borrow::Cow, sync::Arc};

use gpui::App;
use usvg::fontdb;

/// The fonts SVG is laid out and drawn in.
///
/// `generation` changes with each [`add`] in a browser; a face's
/// [`fontdb::ID`] means nothing in another generation's `fonts`.
#[derive(Clone)]
pub struct Database {
    pub fonts: Arc<fontdb::Database>,
    pub generation: u64,
}

/// Hand `fonts` to gpui's text system, and in a browser to [`database`].
pub fn add(cx: &App, fonts: Vec<Cow<'static, [u8]>>) -> gpui::Result<()> {
    #[cfg(target_family = "wasm")]
    web::add(&fonts);
    cx.text_system().add_fonts(fonts)
}

/// The fonts SVG is drawn in. Natively the first call loads the system's,
/// which reads every font file's names: a quarter of a second on a Mac.
pub fn database() -> Database {
    #[cfg(not(target_family = "wasm"))]
    {
        static SYSTEM: std::sync::LazyLock<Database> = std::sync::LazyLock::new(|| {
            let mut fonts = fontdb::Database::new();
            fonts.load_system_fonts();
            Database {
                fonts: Arc::new(fonts),
                generation: 0,
            }
        });
        SYSTEM.clone()
    }
    #[cfg(target_family = "wasm")]
    web::database()
}

#[cfg(target_family = "wasm")]
mod web {
    use std::{
        borrow::Cow,
        sync::{Arc, Mutex, PoisonError},
    };

    use usvg::fontdb::{self, Source};

    use super::Database;

    static FONTS: Mutex<Option<Database>> = Mutex::new(None);

    pub(super) fn add(added: &[Cow<'static, [u8]>]) {
        let mut held = FONTS.lock().unwrap_or_else(PoisonError::into_inner);
        let mut fonts = held
            .as_ref()
            .map_or_else(fontdb::Database::new, |db| (*db.fonts).clone());
        for font in added {
            fonts.load_font_source(Source::Binary(Arc::new(font.clone())));
        }
        let generation = held.as_ref().map_or(0, |db| db.generation) + 1;
        *held = Some(Database {
            fonts: Arc::new(fonts),
            generation,
        });
    }

    pub(super) fn database() -> Database {
        FONTS
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get_or_insert_with(|| Database {
                fonts: Arc::new(fontdb::Database::new()),
                generation: 0,
            })
            .clone()
    }
}
