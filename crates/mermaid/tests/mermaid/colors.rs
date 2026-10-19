//! With `Options::colors`, every corpus entry draws without CSS variables,
//! as an SVG usvg reads.

use std::{fs, path::Path};

use mermaid::{Colors, Metrics, Options, render};

fn colors() -> Colors {
    Colors {
        bg: [0x10, 0x20, 0x30, 0xff],
        fg: [0xf0, 0xf0, 0xf0, 0xff],
        muted: [0x90, 0x90, 0x90, 0xff],
        line: [0xe0, 0xe0, 0xe0, 0xff],
        surface: [0x20, 0x30, 0x40, 0xff],
        surface_alt: [0x30, 0x40, 0x50, 0x80],
        border: [0x70, 0x70, 0x70, 0xff],
        accent: [0x5b, 0x7b, 0xd5, 0xff],
        critical: [0xe4, 0x57, 0x56, 0xff],
        series: [[0x11, 0x22, 0x33, 0xff]; 8],
    }
}

#[test]
fn every_entry_draws_without_css_variables() {
    let options = Options {
        colors: Some(colors()),
        ..Options::default()
    };
    let usvg_options = usvg::Options::default();
    let corpus = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/mermaid/corpus");
    let mut drawn = 0;
    for entry in fs::read_dir(&corpus).expect("corpus") {
        let path = entry.expect("entry").path();
        let entries: Vec<serde_json::Value> =
            serde_json::from_str(&fs::read_to_string(&path).expect("read")).expect("JSON");
        for (i, entry) in entries.iter().enumerate() {
            let source = entry
                .as_str()
                .or_else(|| entry["src"].as_str())
                .expect("a source");
            let Ok(drawing) = render(source, &options, &Metrics) else {
                continue;
            };
            let at = format!("{} #{i}", path.display());
            assert!(!drawing.svg.contains("var("), "{at}: {}", drawing.svg);
            assert!(!drawing.svg.contains("color-mix"), "{at}");
            usvg::Tree::from_str(&drawing.svg, &usvg_options)
                .unwrap_or_else(|error| panic!("{at}: {error}"));
            drawn += 1;
        }
    }
    assert!(drawn > 600, "{drawn}");
}

#[test]
fn a_token_is_its_colour_and_a_mix_is_computed() {
    let options = Options {
        colors: Some(colors()),
        ..Options::default()
    };
    // A cluster title an edge crosses is half `surface_alt` over `bg`.
    let source = "flowchart TB\n  c1-->a2\n  subgraph one\n  a1-->a2\n  end\n  subgraph two\n  b1-->b2\n  end\n  subgraph three\n  c1-->c2\n  end\n  one --> two\n  three --> two\n  two --> c2";
    let svg = render(source, &options, &Metrics).expect("a drawing").svg;
    assert!(svg.contains("#f0f0f0"), "{svg}");
    assert!(svg.contains("#1b2b3bc0"), "{svg}");
}
