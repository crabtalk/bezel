use gpui::{App, Task};
use std::path::PathBuf;

/// Where a page keeps its cookies, storage and cache.
///
/// The default is the platform's own store, shared by every page built with
/// it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DataStore {
    pub(crate) incognito: bool,
    pub(crate) directory: Option<PathBuf>,
    pub(crate) identifier: Option<[u8; 16]>,
}

impl DataStore {
    pub fn new() -> Self {
        Self::default()
    }

    /// Kept in memory and gone with the page. Overrides the directory and
    /// the identifier. On Windows, needs WebView2 Runtime 101.0.1210.39 or
    /// later, and is ignored before it.
    pub fn incognito(mut self) -> Self {
        self.incognito = true;
        self
    }

    /// Windows: the directory the store lives in. Pages built with
    /// one directory share one store. Ignored on macOS.
    pub fn with_directory(mut self, directory: impl Into<PathBuf>) -> Self {
        self.directory = Some(directory.into());
        self
    }

    /// macOS 14 and later: the persistent store with this identifier, a UUID's
    /// bytes. Pages built with one identifier share one store. Ignored
    /// elsewhere, and on earlier macOS, where the page uses the default store.
    pub fn with_identifier(mut self, identifier: [u8; 16]) -> Self {
        self.identifier = Some(identifier);
        self
    }

    /// Clears the cookies, storage and cache this store keeps, whether or not
    /// a page built with it is open. Resolves to whether it was cleared: an
    /// incognito store keeps nothing and resolves `true`. Resolves `false` off
    /// macOS.
    ///
    /// On macOS before 14 an identifier's store is the default one.
    pub fn clear(&self, cx: &App) -> Task<bool> {
        if self.incognito {
            return Task::ready(true);
        }
        let (cleared, answer) = async_channel::bounded(1);
        crate::page::clear_store(self, move |ok| {
            let _ = cleared.try_send(ok);
        });
        cx.foreground_executor()
            .spawn(async move { answer.recv().await.unwrap_or(false) })
    }
}
