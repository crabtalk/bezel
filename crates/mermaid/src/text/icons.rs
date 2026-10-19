//! The host's icons for the render in progress. A label leaves room for an
//! icon only when the host has one by that name, and layout has to know that
//! before anything is drawn.

use std::{cell::RefCell, collections::HashMap, rc::Rc};

/// The SVG markup a host draws icon `name` with, inside a 24×24 view box,
/// or `None` for a name it has no icon for.
pub type IconResolver = Rc<dyn Fn(&str) -> Option<String>>;

thread_local! {
    static RESOLVER: RefCell<Option<IconResolver>> = const { RefCell::new(None) };
    static FOUND: RefCell<HashMap<String, String>> = RefCell::new(HashMap::new());
}

/// Runs `run` with `icons` as the render's resolver.
pub fn with_icons<T>(icons: Option<IconResolver>, run: impl FnOnce() -> T) -> T {
    let outer = RESOLVER.with(|slot| slot.replace(icons));
    let outer_found = FOUND.with(|found| found.replace(HashMap::new()));
    struct Restore(
        Option<Option<IconResolver>>,
        Option<HashMap<String, String>>,
    );
    impl Drop for Restore {
        fn drop(&mut self) {
            let resolver = self.0.take().flatten();
            let found = self.1.take().unwrap_or_default();
            RESOLVER.with(|slot| slot.replace(resolver));
            FOUND.with(|slot| slot.replace(found));
        }
    }
    let _restore = Restore(Some(outer), Some(outer_found));
    run()
}

/// The markup for icon `name`, or `""` when the host has none.
pub fn icon_markup(name: &str) -> String {
    let Some(resolver) = RESOLVER.with(|slot| slot.borrow().clone()) else {
        return String::new();
    };
    if let Some(markup) = FOUND.with(|found| found.borrow().get(name).cloned()) {
        return markup;
    }
    let markup = resolver(name).unwrap_or_default();
    FOUND.with(|found| found.borrow_mut().insert(name.to_string(), markup.clone()));
    markup
}
