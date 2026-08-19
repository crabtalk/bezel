//! The silhouette, as parameters rather than a roster: one radial profile
//! `r(θ)` that every seed lands somewhere inside.

use std::{
    f32::consts::{FRAC_PI_2, FRAC_PI_4, PI, TAU},
    sync::atomic::{AtomicU64, Ordering},
};
use web_time::{SystemTime, UNIX_EPOCH};

/// Points the outline is sampled at. Corners land between samples, so the
/// spline through them is rounded by the sampling itself.
pub const SAMPLES: usize = 64;

/// How close to the centre the outline may dip before the spline through it
/// starts crossing itself.
const FLOOR: f32 = 0.45;

/// One harmonic of the outline: `k` bumps around the circle.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Lobe {
    pub k: u8,
    pub amp: f32,
    pub phase: f32,
}

const NONE: Lobe = Lobe {
    k: 0,
    amp: 0.0,
    phase: 0.0,
};

const fn lobe(k: u8, amp: f32, phase: f32) -> Lobe {
    Lobe { k, amp, phase }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Shape {
    pub lobes: [Lobe; 3],
    /// Sides of the polygon the outline leans toward; under 3 there is none.
    pub sides: u8,
    /// How far it leans — 0 is a circle, 1 the polygon itself.
    pub corner: f32,
    /// Width against height, area held constant.
    pub stretch: f32,
    /// A pull toward a single point at the top, which is what makes a droplet.
    pub taper: f32,
    pub rot: f32,
}

impl Default for Shape {
    fn default() -> Self {
        Self::ROUND
    }
}

impl Shape {
    pub const ROUND: Self = Self {
        lobes: [NONE; 3],
        sides: 0,
        corner: 0.0,
        stretch: 1.0,
        taper: 0.0,
        rot: 0.0,
    };
    pub const EGG: Self = Self {
        stretch: 0.93,
        taper: 0.2,
        ..Self::ROUND
    };
    pub const BEAN: Self = Self {
        lobes: [lobe(2, 0.13, 0.9), NONE, NONE],
        stretch: 1.1,
        rot: 0.5,
        ..Self::ROUND
    };
    pub const DROP: Self = Self {
        stretch: 0.92,
        taper: 0.44,
        ..Self::ROUND
    };
    pub const BLOB: Self = Self {
        lobes: [lobe(3, 0.09, 0.6), lobe(5, 0.035, 2.1), NONE],
        rot: 0.4,
        ..Self::ROUND
    };
    pub const CLOUD: Self = Self {
        lobes: [lobe(5, 0.12, 1.2), lobe(2, 0.05, 0.3), NONE],
        stretch: 1.1,
        ..Self::ROUND
    };
    /// A quarter of the segment past the vertex the polygon term puts at the
    /// top, which is what turns a diamond into a square.
    pub const TILE: Self = Self {
        sides: 4,
        corner: 0.82,
        rot: FRAC_PI_4,
        ..Self::ROUND
    };
    pub const GEM: Self = Self {
        sides: 6,
        corner: 0.72,
        rot: 0.26,
        ..Self::ROUND
    };
    pub const SHARD: Self = Self {
        sides: 3,
        corner: 0.62,
        stretch: 1.04,
        ..Self::ROUND
    };
    pub const SUN: Self = Self {
        lobes: [lobe(8, 0.11, 0.0), NONE, NONE],
        ..Self::ROUND
    };

    /// The presets, for a picker that wants to name them.
    pub const PRESETS: [(&'static str, Self); 10] = [
        ("round", Self::ROUND),
        ("egg", Self::EGG),
        ("bean", Self::BEAN),
        ("blob", Self::BLOB),
        ("cloud", Self::CLOUD),
        ("drop", Self::DROP),
        ("tile", Self::TILE),
        ("gem", Self::GEM),
        ("shard", Self::SHARD),
        ("sun", Self::SUN),
    ];

    /// A fresh silhouette, unrelated to the last one.
    pub fn random() -> Self {
        static COUNT: AtomicU64 = AtomicU64::new(0);
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0);
        Self::from(nanos ^ COUNT.fetch_add(0x9e37_79b9_7f4a_7c15, Ordering::Relaxed))
    }

    /// The outline's distance from the centre at `theta`, before stretch.
    pub fn radius(&self, theta: f32) -> f32 {
        let a = theta + self.rot;
        let mut r = 1.0;
        if self.sides >= 3 {
            // Shifted a quarter turn so a vertex sits at the top, which is
            // where every polygon anyone draws by hand puts one.
            let seg = TAU / self.sides as f32;
            let phi = ((a + FRAC_PI_2) % seg + seg) % seg - seg / 2.0;
            r += ((seg / 2.0).cos() / phi.cos() - 1.0) * self.corner;
        }
        for l in &self.lobes {
            if l.k >= 2 {
                r += l.amp * (l.k as f32 * a + l.phase).cos();
            }
        }
        // Read off `theta` rather than the rotated angle: a point belongs at the
        // top of the head, and one swung round to the side reads as an arrow.
        // Falls away steeply, so a tapered body keeps its own curve at the sides.
        let up = (1.0 - theta.sin()) * 0.5;
        r += self.taper * up.powi(6);
        r.max(FLOOR)
    }

    /// The outline in unit space, centred on the origin.
    pub fn outline(&self) -> [(f32, f32); SAMPLES] {
        std::array::from_fn(|i| {
            let a = TAU * i as f32 / SAMPLES as f32;
            let r = self.radius(a);
            (r * a.cos() * self.stretch, r * a.sin() / self.stretch)
        })
    }

    /// How far the outline sits in the direction of `(x, y)` — what anything
    /// placed on the body has to measure itself against.
    pub fn reach(&self, x: f32, y: f32) -> f32 {
        let a = y.atan2(x);
        let r = self.radius(a);
        (r * a.cos() * self.stretch).hypot(r * a.sin() / self.stretch)
    }
}

/// SplitMix64 — one multiply-xor round per value, and every seed is a legal
/// starting point, which is what makes `random()` and a name the same thing.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    fn unit(&mut self) -> f32 {
        (self.next() >> 40) as f32 / 16_777_216.0
    }

    fn range(&mut self, lo: f32, hi: f32) -> f32 {
        lo + self.unit() * (hi - lo)
    }

    fn int(&mut self, lo: u8, hi: u8) -> u8 {
        lo + (self.unit() * (hi - lo + 1) as f32) as u8
    }
}

impl From<u64> for Shape {
    fn from(seed: u64) -> Self {
        let mut rng = Rng(seed);
        let sides = if rng.unit() < 0.34 { rng.int(3, 8) } else { 0 };
        let corner = if sides >= 3 { rng.range(0.2, 0.8) } else { 0.0 };

        // The polygon term only ever pulls the outline inward, so what it takes
        // is what the harmonics may not spend.
        let waist = if sides >= 3 {
            1.0 + corner * ((PI / sides as f32).cos() - 1.0)
        } else {
            1.0
        };
        let mut budget = (waist - FLOOR).clamp(0.0, 0.26);

        let count = rng.int(1, 3);
        let lobes = std::array::from_fn(|i| {
            if i as u8 >= count || budget <= 0.01 {
                return Lobe::default();
            }
            // Squared, so most bodies are gently lobed and a spiky one is a
            // find — a uniform draw makes a starburst of nearly every seed.
            let k = 2 + (rng.unit().powi(2) * 7.0) as u8;
            // The same amplitude at k=8 that reads as a curve at k=3 reads as
            // teeth, so what a lobe may spend falls as it gets finer.
            let amp = budget * rng.range(0.35, 0.9) * (3.0 / k as f32).min(1.0);
            budget -= amp;
            lobe(k, amp, rng.range(0.0, TAU))
        });

        Self {
            lobes,
            sides,
            corner,
            stretch: rng.range(0.88, 1.14),
            taper: if rng.unit() < 0.28 {
                rng.range(0.08, 0.3)
            } else {
                0.0
            },
            rot: rng.range(0.0, TAU),
        }
    }
}

impl From<&str> for Shape {
    fn from(name: &str) -> Self {
        Self::from(seed(name))
    }
}

/// FNV-1a over a canonical form of the name — lowercased, outer whitespace
/// dropped and inner runs collapsed to one space — so the same person keeps the
/// same face across whatever the keyboard did.
///
/// Composed and decomposed spellings still part ways: `café` written with `é`
/// and with `e` + a combining acute are one name to a reader and two here.
/// Normalizing them together needs a Unicode table, which is a dependency this
/// crate does not carry.
pub fn seed(name: &str) -> u64 {
    let feed = |h: u64, b: u8| (h ^ b as u64).wrapping_mul(0x0000_0100_0000_01b3);
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    let (mut started, mut gap) = (false, false);
    let mut buf = [0u8; 4];
    for c in name.chars().flat_map(char::to_lowercase) {
        if c.is_whitespace() {
            // Only between characters, so leading and trailing runs never land.
            gap = started;
            continue;
        }
        if gap {
            h = feed(h, b' ');
            gap = false;
        }
        for b in c.encode_utf8(&mut buf).as_bytes() {
            h = feed(h, *b);
        }
        started = true;
    }
    h
}
