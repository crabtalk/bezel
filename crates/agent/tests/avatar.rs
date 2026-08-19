//! Fidelity pins against the blobatar reference (their gen2 golden fixture,
//! regenerated here with the original sources under bun), plus the two
//! properties the whole feature exists for.

use agent::avatar::{
    color::{self, Oklch},
    geometry::{Art, Path2D, Seg},
};

/// JS `Math.round` semantics: half rounds toward +∞, and `-0` prints as `0`.
fn r2(v: f64) -> String {
    let n = ((v * 100.0) + 0.5).floor() / 100.0;
    if n == 0.0 { "0".into() } else { format!("{n}") }
}

fn hex(c: Oklch) -> String {
    let [r, g, b] = color::to_rgb8(c);
    format!("{r:02x}{g:02x}{b:02x}")
}

fn path(p: &Path2D) -> String {
    let mut out = String::new();
    let mut prev = None;
    for seg in &p.segs {
        match seg {
            Seg::Move { x, y } => out.push_str(&format!("M{} {}", r2(*x), r2(*y))),
            Seg::Line { x, y } => {
                let (px, py) = prev.unwrap();
                if *y == py {
                    out.push_str(&format!("H{}", r2(*x)));
                } else if *x == px {
                    out.push_str(&format!("V{}", r2(*y)));
                } else {
                    out.push_str(&format!("L{} {}", r2(*x), r2(*y)));
                }
            }
            Seg::Quad { cx, cy, x, y } => {
                out.push_str(&format!("Q{} {} {} {}", r2(*cx), r2(*cy), r2(*x), r2(*y)));
            }
            Seg::Cubic {
                c1x,
                c1y,
                c2x,
                c2y,
                x,
                y,
            } => out.push_str(&format!(
                "C{} {} {} {} {} {}",
                r2(*c1x),
                r2(*c1y),
                r2(*c2x),
                r2(*c2y),
                r2(*x),
                r2(*y)
            )),
        }
        prev = Some(match seg {
            Seg::Move { x, y } | Seg::Line { x, y } => (*x, *y),
            Seg::Quad { x, y, .. } | Seg::Cubic { x, y, .. } => (*x, *y),
        });
    }
    out.push('Z');
    out
}

/// The reference's exact serialization, so a diff against their golden
/// fixture is byte-for-byte.
fn markup(name: &str) -> String {
    let art = Art::from_name(name);
    let mut out = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 100 100\"><g fill=\"#{}\">",
        hex(art.head)
    );
    for c in &art.petals {
        out.push_str(&format!(
            "<circle cx=\"{}\" cy=\"{}\" r=\"{}\"/>",
            r2(c.cx),
            r2(c.cy),
            r2(c.r)
        ));
    }
    for p in &art.extra {
        out.push_str(&format!("<path d=\"{}\"/>", path(p)));
    }
    out.push_str(&format!(
        "<path d=\"{}\"/></g><g fill=\"#{}\">",
        path(&art.body),
        hex(art.eye)
    ));
    for e in &art.eyes {
        out.push_str(&format!("<path d=\"{}\"/>", path(e)));
    }
    out.push_str("</g></svg>");
    out
}

/// Seed → markup, byte-identical to the reference implementation. A diff here
/// is a breaking identity change and must match a deliberate upstream move.
#[test]
fn golden_markup() {
    let cases = [
        (
            "alain",
            "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 100 100\"><g fill=\"#d0d897\"><path d=\"M88.1 50.49C88.1 73.31 73.75 87.6 50.83 87.6C27.91 87.6 13.56 73.31 13.56 50.49C13.56 27.67 27.91 13.39 50.83 13.39C73.75 13.39 88.1 27.67 88.1 50.49Z\"/></g><g fill=\"#0f1006\"><path d=\"M41.86 50.24C39.96 59.57 39.93 59.62 36.16 58.86C32.39 58.09 32.38 58.03 34.28 48.7C36.17 39.36 36.21 39.31 39.97 40.08C43.74 40.84 43.75 40.9 41.86 50.24Z\"/><path d=\"M64.59 48.75C62.81 57.79 62.78 57.85 59.55 57.21C56.33 56.57 56.32 56.51 58.1 47.47C59.89 38.42 59.92 38.37 63.14 39.01C66.37 39.64 66.38 39.7 64.59 48.75Z\"/></g></svg>",
        ),
        (
            "🦊",
            "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 100 100\"><g fill=\"#dfc3fd\"><circle cx=\"37.16\" cy=\"49.96\" r=\"20.18\"/><circle cx=\"63.66\" cy=\"49.96\" r=\"20.18\"/><path d=\"M37.16 29.78H63.66V70.14H37.16Z\"/></g><g fill=\"#120d16\"><path d=\"M45.57 51.11C45.15 59.11 45.09 59.23 41.68 59.05C38.28 58.87 38.23 58.75 38.65 50.75C39.08 42.75 39.13 42.63 42.54 42.81C45.95 42.99 45.99 43.11 45.57 51.11Z\"/><path d=\"M63.24 51.37C62.91 58.35 62.87 58.45 60.19 58.33C57.5 58.2 57.47 58.1 57.8 51.12C58.13 44.14 58.17 44.04 60.85 44.16C63.53 44.29 63.57 44.39 63.24 51.37Z\"/></g></svg>",
        ),
        (
            "user-0",
            "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 100 100\"><g fill=\"#e5d1dd\"><path d=\"M82.21 49.68C82.58 56.67 78.86 66.23 73.43 71.68C68 77.13 57.94 81.99 49.61 82.35C41.28 82.71 29.74 79.28 23.46 73.84C17.18 68.39 11.42 57.24 11.94 49.68C12.47 42.11 20.36 32.87 26.64 28.45C32.92 24.04 42.19 22.97 49.61 23.19C57.04 23.4 65.76 25.33 71.19 29.74C76.62 34.16 81.83 42.69 82.21 49.68Z\"/></g><g fill=\"#150c12\"><path d=\"M42.11 50.88C41.63 56.98 41.63 56.98 38.96 56.77C36.3 56.57 36.3 56.57 36.77 50.46C37.25 44.36 37.25 44.36 39.92 44.56C42.59 44.77 42.59 44.77 42.11 50.88Z\"/><path d=\"M58.04 51.28C57.53 57.34 57.53 57.34 55.24 57.14C52.95 56.95 52.95 56.95 53.46 50.89C53.98 44.83 53.98 44.83 56.27 45.02C58.56 45.22 58.56 45.22 58.04 51.28Z\"/></g></svg>",
        ),
        (
            "Sara",
            "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 100 100\"><g fill=\"#e5d1dd\"><path d=\"M87.6 49.99C88.26 58 84.14 70.82 78.09 75.93C72.03 81.04 60.15 80.72 51.25 80.67C42.36 80.63 31.05 80.77 24.7 75.66C18.36 70.54 12.59 57.96 13.19 49.99C13.79 42.02 21.97 32.36 28.32 27.82C34.66 23.28 43.62 22.76 51.25 22.77C58.89 22.78 68.07 23.34 74.13 27.88C80.18 32.42 86.94 41.98 87.6 49.99Z\"/></g><g fill=\"#160c12\"><path d=\"M44.22 49.7C45.03 54.95 44.81 55.81 42.6 56.15C40.39 56.49 39.93 55.73 39.12 50.48C38.32 45.23 38.54 44.37 40.75 44.04C42.96 43.7 43.42 44.45 44.22 49.7Z\"/><path d=\"M59.57 50C60.07 54.22 59.84 54.9 57.86 55.14C55.87 55.37 55.49 54.76 55 50.54C54.5 46.32 54.73 45.64 56.71 45.41C58.7 45.17 59.07 45.78 59.57 50Z\"/></g></svg>",
        ),
        (
            "café-4",
            "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 100 100\"><g fill=\"#99b54d\"><path d=\"M84.86 49.2C84.86 73.24 73.78 84.51 50.14 84.51C26.51 84.51 15.43 73.24 15.43 49.2C15.43 25.17 26.51 13.9 50.14 13.9C73.78 13.9 84.86 25.17 84.86 49.2Z\"/></g><g fill=\"#0e1107\"><path d=\"M42.38 49.55C42.49 55.95 42.49 55.95 39.43 56C36.37 56.05 36.37 56.05 36.26 49.65C36.15 43.26 36.15 43.26 39.21 43.2C42.27 43.15 42.27 43.15 42.38 49.55Z\"/><path d=\"M61.22 50.08C61.55 56.64 61.55 56.64 57.98 56.82C54.4 57 54.4 57 54.07 50.44C53.74 43.89 53.74 43.89 57.32 43.71C60.89 43.53 60.89 43.53 61.22 50.08Z\"/></g></svg>",
        ),
        (
            "  Mixed Case 9  ",
            "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 100 100\"><g fill=\"#c6d2ff\"><path d=\"M82.89 46.8C86.92 75.9 86.37 76.58 55.24 80.9C24.12 85.22 23.4 84.71 19.36 55.61C15.33 26.51 15.88 25.83 47.01 21.52C78.13 17.2 78.85 17.71 82.89 46.8Z\"/></g><g fill=\"#0c0f18\"><path d=\"M46.8 50C45.63 57.41 45.61 57.44 42.38 56.93C39.15 56.42 39.14 56.38 40.31 48.98C41.47 41.58 41.5 41.54 44.73 42.05C47.96 42.56 47.97 42.6 46.8 50Z\"/><path d=\"M64.76 51.4C63.05 60.04 63.01 60.08 59.07 59.3C55.12 58.52 55.11 58.47 56.82 49.83C58.53 41.18 58.56 41.14 62.51 41.92C66.45 42.7 66.47 42.75 64.76 51.4Z\"/></g></svg>",
        ),
        (
            "Grace Hopper",
            "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 100 100\"><g fill=\"#81a4fd\"><path d=\"M90.58 49.11C90.43 58.23 83.5 70.28 76.52 76.04C69.53 81.81 57.17 84.45 48.67 83.7C40.17 82.94 30.76 77.27 25.52 71.51C20.28 65.74 18.3 57.61 17.23 49.11C16.15 40.6 13.83 27.01 19.07 20.46C24.31 13.92 38.95 9.68 48.67 9.83C58.4 9.97 70.42 14.76 77.4 21.31C84.39 27.86 90.72 39.98 90.58 49.11Z\"/></g><g fill=\"#0b0f18\"><path d=\"M42.01 50.87C42.14 58.68 42.14 58.68 38.74 58.74C35.34 58.8 35.34 58.8 35.2 50.99C35.07 43.18 35.07 43.18 38.47 43.12C41.87 43.06 41.87 43.06 42.01 50.87Z\"/><path d=\"M60.45 51.6C60.27 61.68 60.27 61.68 56.5 61.61C52.73 61.55 52.73 61.55 52.91 51.46C53.09 41.38 53.09 41.38 56.86 41.44C60.63 41.51 60.63 41.51 60.45 51.6Z\"/></g></svg>",
        ),
        (
            "Team Rocket 3",
            "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 100 100\"><g fill=\"#1a89e4\"><path d=\"M83.42 49.43C83.42 70.88 70.83 84.12 50.42 84.12C30 84.12 17.41 70.88 17.41 49.43C17.41 27.98 30 14.74 50.42 14.74C70.83 14.74 83.42 27.98 83.42 49.43Z\"/></g><g fill=\"#091018\"><path d=\"M41.52 48.9C42.62 54.44 42.56 54.63 39.75 55.18C36.94 55.74 36.82 55.58 35.73 50.05C34.63 44.52 34.69 44.32 37.5 43.77C40.31 43.22 40.43 43.37 41.52 48.9Z\"/><path d=\"M60.9 49.45C61.75 53.72 61.7 53.86 59.4 54.32C57.1 54.78 57.01 54.66 56.16 50.39C55.31 46.12 55.36 45.98 57.66 45.52C59.96 45.07 60.06 45.18 60.9 49.45Z\"/></g></svg>",
        ),
    ];
    for (name, expected) in cases {
        assert_eq!(
            markup(name),
            expected,
            "seed {name:?} drifted from the reference"
        );
    }
}

#[test]
fn deterministic_and_distinct() {
    assert_eq!(Art::from_name("Sara"), Art::from_name("Sara"));
    assert_ne!(Art::from_name("Sara"), Art::from_name("Dan"));
    assert_ne!(Art::from_name("Grace Hopper"), Art::from_name("Grace"));
}

/// Normalization: trim + lowercase, so a person's identity survives the
/// keyboard (NFC is deliberately skipped).
#[test]
fn normalized() {
    assert_eq!(
        Art::from_name("  MIXED CASE 9  "),
        Art::from_name("mixed case 9")
    );
    assert_eq!(
        Art::from_name("Alain@Example.com"),
        Art::from_name("alain@example.com")
    );
}

/// Every band in the table is reachable, and the shape vocabulary is complete.
#[test]
fn all_silhouettes_reachable() {
    let mut seen = std::collections::HashSet::new();
    for i in 0..2000 {
        seen.insert(Art::from_name(&format!("histogram-{i}")).shape);
    }
    for name in [
        "round", "organic", "boxy", "capsule", "nub", "cloud", "droplet", "hexagon", "sun",
        "triangle",
    ] {
        assert!(seen.contains(name), "silhouette {name:?} never appears");
    }
}

/// The contrast floors hold under the ported luminance convention.
#[test]
fn palette_floors() {
    let dark = Oklch {
        l: 0.145,
        c: 0.0,
        h: 0.0,
    };
    for i in 0..200 {
        let art = Art::from_name(&format!("s{i}"));
        assert!(
            color::contrast(art.head, dark) >= 1.5,
            "s{i} head vanishes on a dark page"
        );
        assert!(
            color::contrast(art.eye, art.head) >= 4.5,
            "s{i} eyes lose their face"
        );
    }
}
