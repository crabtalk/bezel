use gallery::{PLANNED_BODIES, Section, TABS};

fn all_sections() -> impl Iterator<Item = &'static Section> {
    TABS.iter()
        .flat_map(|tab| tab.groups)
        .flat_map(|group| group.sections)
}

/// Two rows with the same key would open the same page, and the rail would
/// highlight both.
#[test]
fn rail_keys_are_unique() {
    let mut seen = std::collections::HashSet::new();
    for section in all_sections() {
        assert!(
            seen.insert(section.key),
            "duplicate rail key {}",
            section.key
        );
    }
}

/// The header prints a source path as documentation. A moved or renamed
/// file turns that into a lie, silently — so check every one resolves.
#[test]
fn every_section_names_a_file_that_exists() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|apps| apps.parent())
        .expect("workspace root");
    for section in all_sections() {
        let Some(source) = section.source else {
            continue;
        };
        let path = root.join(source);
        assert!(
            path.exists(),
            "{} points at {}, which does not exist",
            section.key,
            path.display()
        );
    }
}

/// The rail and [`Gallery::section_body`] have to agree on what is unbuilt.
/// Both directions bite: a `planned()` row with no arm renders a blank
/// page, and a component that gets built but keeps its TODO arm documents
/// itself as missing while sitting right there in the crate.
///
/// Both sides are empty as of the pagination commit — every row has a
/// source. That is the assertion passing, not the assertion being vacuous:
/// the day something is planned again, it has to be declared on both sides
/// or this fails.
#[test]
fn planned_rows_and_todo_pages_agree() {
    let mut rows: Vec<_> = all_sections()
        .filter(|section| section.source.is_none())
        .map(|section| section.key)
        .collect();
    let mut pages = PLANNED_BODIES.to_vec();
    rows.sort_unstable();
    pages.sort_unstable();
    assert_eq!(
        rows, pages,
        "rail rows without a source must match the TODO pages exactly"
    );
}
