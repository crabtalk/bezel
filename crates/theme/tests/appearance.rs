use theme::{
    Appearance,
    appearance::{AppearanceMode, resolve},
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
