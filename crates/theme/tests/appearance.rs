use theme::{
    Appearance,
    appearance::{AppearanceMode, reports_the_os, resolve},
};

#[test]
fn system_mode_follows_the_os() {
    assert_eq!(
        resolve(AppearanceMode::System, Appearance::Light),
        Appearance::Light
    );
    assert_eq!(
        resolve(AppearanceMode::System, Appearance::Dark),
        Appearance::Dark
    );
}

#[test]
fn pinned_modes_ignore_the_os() {
    for system in [Appearance::Light, Appearance::Dark] {
        assert_eq!(resolve(AppearanceMode::Light, system), Appearance::Light);
        assert_eq!(resolve(AppearanceMode::Dark, system), Appearance::Dark);
    }
}

#[test]
fn default_mode_is_system() {
    assert_eq!(AppearanceMode::default(), AppearanceMode::System);
}

/// The setting round-trips through the settings file as a lowercase string.
#[test]
fn mode_serialises_stably() {
    for (mode, json) in [
        (AppearanceMode::System, "\"system\""),
        (AppearanceMode::Light, "\"light\""),
        (AppearanceMode::Dark, "\"dark\""),
    ] {
        assert_eq!(serde_json::to_string(&mode).unwrap(), json);
        assert_eq!(
            serde_json::from_str::<AppearanceMode>(json).unwrap(),
            mode,
            "{json} should parse back"
        );
    }
}

/// What a window says about the appearance is only the OS's answer while
/// nothing is pinned. A pinned mode sets `NSApplication.appearance`, and every
/// window then reports that override straight back — take it for the system's
/// own and the stored value becomes the mode itself, so the trip back to
/// `System` resolves to the appearance just left and the window keeps the
/// vibrancy that went with it.
#[test]
fn only_system_mode_hears_the_os() {
    assert!(reports_the_os(AppearanceMode::System));
    assert!(!reports_the_os(AppearanceMode::Light));
    assert!(!reports_the_os(AppearanceMode::Dark));
}
