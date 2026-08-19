//! The blobatar geometry: seed hash, trait reader, five path primitives and
//! the ten-silhouette layout that composes them. Pure math — no gpui, f64
//! throughout so the numbers match the reference exactly.

use super::color::Oklch;

const SEP: u8 = 0xff;

fn feed(mut h: u32, bytes: &[u8]) -> u32 {
    for &b in bytes {
        h = (h ^ b as u32).wrapping_mul(3432918353);
        h = h.rotate_left(13);
    }
    h
}

/// Murmur3 fmix32 — a bijection on uint32 with full avalanche.
fn finalize(mut h: u32) -> u32 {
    h ^= h >> 16;
    h = h.wrapping_mul(2246822507);
    h ^= h >> 13;
    h = h.wrapping_mul(3266489909);
    h ^ (h >> 16)
}

fn stream(state: u32, key: &str) -> f64 {
    let h = feed(state, &[SEP]);
    finalize(feed(h, key.as_bytes())) as f64 / 4294967296.0
}

/// Seed state: normalized name (trim + lowercase — NFC deliberately skipped),
/// hashed over its UTF-8 bytes with the UTF-16 length mixed in, exactly like
/// the reference.
fn seed_state(name: &str) -> u32 {
    let s = name.trim().to_lowercase();
    let utf16 = s.encode_utf16().count() as u32;
    feed(1779033703 ^ utf16, s.as_bytes())
}

/// A trait reader. Every value is addressed by a string key rather than drawn
/// from a sequential stream, so trait keys are an append-only namespace: each
/// key's value derives from the same seed state, independent of every other
/// key and of read order.
#[derive(Clone, Copy)]
pub struct Reader {
    state: u32,
}

impl Reader {
    fn new(name: &str) -> Self {
        Self {
            state: seed_state(name),
        }
    }

    fn raw(&self, key: &str) -> f64 {
        stream(self.state, key)
    }

    fn num(&self, key: &str, min: f64, max: f64) -> f64 {
        min + self.raw(key) * (max - min)
    }

    fn int(&self, key: &str, min: i32, max: i32) -> i32 {
        min + (self.raw(key) * (max - min + 1) as f64).floor() as i32
    }

    fn jitter(&self, key: &str, amount: f64) -> f64 {
        (self.raw(key) * 2.0 - 1.0) * amount
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Seg {
    Move {
        x: f64,
        y: f64,
    },
    Line {
        x: f64,
        y: f64,
    },
    Quad {
        cx: f64,
        cy: f64,
        x: f64,
        y: f64,
    },
    Cubic {
        c1x: f64,
        c1y: f64,
        c2x: f64,
        c2y: f64,
        x: f64,
        y: f64,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct Path2D {
    pub segs: Vec<Seg>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Circle {
    pub cx: f64,
    pub cy: f64,
    pub r: f64,
}

/// A superellipse: |x/rx|^n + |y/ry|^n = 1, each quadrant one cubic Bézier
/// whose control offset is chosen so the curve passes through the 45° point.
fn superellipse(cx: f64, cy: f64, rx: f64, ry: f64, n: f64, rot: f64) -> Path2D {
    // Above n≈5.55 the control offset exceeds the radius and the curve bulges
    // outside its bounding box; clamping keeps it within stated bounds.
    let k = (8.0 * 2.0_f64.powf(-1.0 / n) - 4.0) / 3.0;
    let k = k.min(1.0);
    let ak = rx * k;
    let bk = ry * k;

    // Anchor, control, control — walking the four quadrants.
    let pts: [(f64, f64); 13] = [
        (rx, 0.0),
        (rx, bk),
        (ak, ry),
        (0.0, ry),
        (-ak, ry),
        (-rx, bk),
        (-rx, 0.0),
        (-rx, -bk),
        (-ak, -ry),
        (0.0, -ry),
        (ak, -ry),
        (rx, -bk),
        (rx, 0.0),
    ];

    let t = rot.to_radians();
    let (cos, sin) = (t.cos(), t.sin());
    let at = |(x, y): (f64, f64)| (cx + x * cos - y * sin, cy + x * sin + y * cos);

    let mut segs = vec![Seg::Move {
        x: at(pts[0]).0,
        y: at(pts[0]).1,
    }];
    for i in (1..13).step_by(3) {
        let c1 = at(pts[i]);
        let c2 = at(pts[i + 1]);
        let end = at(pts[i + 2]);
        segs.push(Seg::Cubic {
            c1x: c1.0,
            c1y: c1.1,
            c2x: c2.0,
            c2y: c2.1,
            x: end.0,
            y: end.1,
        });
    }
    Path2D { segs }
}

/// An organic closed curve: radii sampled around a circle, joined by a closed
/// Catmull-Rom spline converted to cubic Béziers.
fn blob_path(cx: f64, cy: f64, rx: f64, ry: f64, radii: &[f64], rot: f64) -> Path2D {
    let n = radii.len();
    let t0 = rot.to_radians();
    let p: Vec<(f64, f64)> = radii
        .iter()
        .enumerate()
        .map(|(i, m)| {
            let a = t0 + 2.0 * std::f64::consts::PI * i as f64 / n as f64;
            (cx + rx * m * a.cos(), cy + ry * m * a.sin())
        })
        .collect();

    let at = |i: isize| p[((i % n as isize) + n as isize) as usize % n];

    let mut segs = vec![Seg::Move {
        x: at(0).0,
        y: at(0).1,
    }];
    for i in 0..n as isize {
        let (x0, y0) = at(i - 1);
        let (x1, y1) = at(i);
        let (x2, y2) = at(i + 1);
        let (x3, y3) = at(i + 2);
        segs.push(Seg::Cubic {
            c1x: x1 + (x2 - x0) / 6.0,
            c1y: y1 + (y2 - y0) / 6.0,
            c2x: x2 - (x3 - x1) / 6.0,
            c2y: y2 - (y3 - y1) / 6.0,
            x: x2,
            y: y2,
        });
    }
    Path2D { segs }
}

/// A regular polygon with rounded corners. Corners are cut back along both
/// adjoining edges and joined with a quadratic through the vertex, which puts
/// the whole outline inside the polygon's convex hull.
fn polygon(cx: f64, cy: f64, rx: f64, ry: f64, sides: i32, round: f64, rot: f64) -> Path2D {
    // Halved because the cut is taken from both ends of every edge.
    let k = if round > 0.0 {
        if round < 1.0 { round / 2.0 } else { 0.5 }
    } else {
        0.0
    };
    // −90° so a vertex sits at the top.
    let t0 = rot.to_radians() - std::f64::consts::FRAC_PI_2;
    let v: Vec<(f64, f64)> = (0..sides)
        .map(|i| {
            let a = t0 + 2.0 * std::f64::consts::PI * i as f64 / sides as f64;
            (cx + rx * a.cos(), cy + ry * a.sin())
        })
        .collect();

    let at = |i: isize| v[((i % sides as isize) + sides as isize) as usize % sides as usize];
    let cut = |i: isize, j: isize| {
        let (x0, y0) = at(i);
        let (x1, y1) = at(j);
        (x0 + (x1 - x0) * k, y0 + (y1 - y0) * k)
    };

    let (mx, my) = cut(0, -1);
    let mut segs = vec![Seg::Move { x: mx, y: my }];
    for i in 0..sides as isize {
        let (x, y) = at(i);
        let (ex, ey) = cut(i, i + 1);
        segs.push(Seg::Quad {
            cx: x,
            cy: y,
            x: ex,
            y: ey,
        });
        // The straight run to the next corner's cut; omitted when the cuts
        // meet, so a fully rounded polygon emits no zero-length lines.
        if k < 0.5 {
            let (lx, ly) = cut(i + 1, i);
            segs.push(Seg::Line { x: lx, y: ly });
        }
    }
    Path2D { segs }
}

/// The straight run of a capsule — a plain box, drawn with the two cap
/// circles the capsule already decorates with.
fn box_path(cx: f64, cy: f64, rx: f64, ry: f64) -> Path2D {
    Path2D {
        segs: vec![
            Seg::Move {
                x: cx - rx,
                y: cy - ry,
            },
            Seg::Line {
                x: cx + rx,
                y: cy - ry,
            },
            Seg::Line {
                x: cx + rx,
                y: cy + ry,
            },
            Seg::Line {
                x: cx - rx,
                y: cy + ry,
            },
        ],
    }
}

/// The taper of a droplet: the two tangents from an apex to the body ellipse,
/// eased with a quadratic through the apex.
fn taper(cx: f64, cy: f64, rx: f64, ry: f64, tip: f64) -> Path2D {
    let t = tip.max(1.05);
    let tx = rx * (1.0 - 1.0 / (t * t)).sqrt();
    let ty = cy - ry / t;
    let apex = cy - t * ry;
    let px = tx * 0.14;
    let py = ty + 0.86 * (apex - ty);
    Path2D {
        segs: vec![
            Seg::Move { x: cx - tx, y: ty },
            Seg::Line { x: cx - px, y: py },
            Seg::Quad {
                cx,
                cy: apex,
                x: cx + px,
                y: py,
            },
            Seg::Line { x: cx + tx, y: ty },
        ],
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Body {
    pub cx: f64,
    pub cy: f64,
    pub rx: f64,
    pub ry: f64,
    pub n: f64,
    pub rot: f64,
    pub radii: Vec<f64>,
    pub sides: Option<i32>,
    pub round: Option<f64>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ellipse {
    pub cx: f64,
    pub cy: f64,
    pub rx: f64,
    pub ry: f64,
}

#[derive(Default)]
pub struct Deco {
    pub petals: Vec<Circle>,
    pub extra: Vec<Path2D>,
}

/// The ten silhouettes. A shape owns how much of the frame its core body
/// takes, how it patches that body, what room it leaves the eyes, what it
/// decorates with, and which path primitive traces it. What it does not carry
/// is its threshold — that is the band table's property.
#[derive(Clone, Copy, PartialEq)]
pub enum Silhouette {
    Round,
    Organic,
    Boxy,
    Capsule,
    Nub,
    Cloud,
    Droplet,
    Hexagon,
    Sun,
    Triangle,
}

impl Silhouette {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Round => "round",
            Self::Organic => "organic",
            Self::Boxy => "boxy",
            Self::Capsule => "capsule",
            Self::Nub => "nub",
            Self::Cloud => "cloud",
            Self::Droplet => "droplet",
            Self::Hexagon => "hexagon",
            Self::Sun => "sun",
            Self::Triangle => "triangle",
        }
    }

    fn core(&self) -> f64 {
        match self {
            Self::Round => 1.0,
            Self::Organic => 0.98,
            Self::Boxy => 0.86,
            Self::Capsule => 1.02,
            Self::Nub => 0.88,
            Self::Cloud => 0.78,
            Self::Droplet => 0.78,
            Self::Hexagon => 1.05,
            Self::Sun => 0.7,
            Self::Triangle => 1.15,
        }
    }

    fn body_patch(&self, t: &Reader, b: &mut Body) {
        match self {
            Self::Boxy => {
                b.n = t.num("body.n", 3.4, 6.0);
                b.rot = t.num("body.rot", -20.0, 20.0);
            }
            Self::Capsule => b.ry *= t.num("capsule.squat", 0.55, 0.68),
            // Shifted down by what the taper adds above, so the whole
            // silhouette sits centred. `n` pinned to a true ellipse, which is
            // the curve the taper is tangent to.
            Self::Droplet => {
                b.cy += 0.22 * b.ry;
                b.n = 2.0;
            }
            Self::Hexagon => {
                b.sides = Some(6);
                b.rot = t.num("body.rot", -12.0, 12.0);
                b.round = Some(t.num("poly.round", 0.24, 0.5));
            }
            Self::Triangle => {
                b.sides = Some(3);
                b.rot = t.num("body.rot", -5.0, 5.0);
                b.round = Some(t.num("poly.round", 0.24, 0.5));
            }
            _ => {}
        }
    }

    /// The region the eyes must fit inside. `None` is the body itself.
    fn face(&self, b: &Body) -> Option<Ellipse> {
        let shrunk = |k: f64| Ellipse {
            cx: b.cx,
            cy: b.cy,
            rx: b.rx * k,
            ry: b.ry * k,
        };
        match self {
            Self::Organic | Self::Cloud => {
                let m = b.radii.iter().cloned().fold(f64::INFINITY, f64::min);
                Some(shrunk(m * 0.95))
            }
            Self::Capsule => Some(shrunk(0.94)),
            Self::Hexagon => Some(shrunk(0.84)),
            Self::Droplet => Some(Ellipse {
                cx: b.cx,
                cy: b.cy + b.ry * 0.05,
                rx: b.rx * 0.88,
                ry: b.ry * 0.88,
            }),
            Self::Triangle => Some(Ellipse {
                cx: b.cx,
                cy: b.cy + b.ry * 0.1,
                rx: b.rx * 0.54,
                ry: b.ry * 0.36,
            }),
            _ => None,
        }
    }

    fn decorate(&self, t: &Reader, b: &Body, out: &mut Deco) {
        match self {
            Self::Capsule => {
                for s in [-1.0, 1.0] {
                    out.petals.push(Circle {
                        cx: b.cx + s * (b.rx - b.ry),
                        cy: b.cy,
                        r: b.ry,
                    });
                }
            }
            Self::Nub => {
                for i in 0..t.int("nub.n", 1, 2) {
                    let a = t.num(&format!("nub.a{i}"), 0.0, 2.0 * std::f64::consts::PI);
                    out.petals.push(Circle {
                        cx: b.cx + a.cos() * b.rx * 0.88,
                        cy: b.cy + a.sin() * b.rx * 0.88,
                        r: b.rx * t.num(&format!("nub.r{i}"), 0.24, 0.4),
                    });
                }
            }
            Self::Cloud => {
                let count = t.int("cloud.n", 4, 6);
                for i in 0..count {
                    let a = std::f64::consts::PI
                        + (std::f64::consts::PI * (i as f64 + 0.5)) / count as f64;
                    out.petals.push(Circle {
                        cx: b.cx + a.cos() * b.rx * 0.8,
                        cy: b.cy + a.sin() * b.rx * 0.5,
                        r: b.rx * t.num(&format!("cloud.r{i}"), 0.44, 0.62),
                    });
                }
            }
            Self::Droplet => {
                out.extra.push(taper(
                    b.cx,
                    b.cy,
                    b.rx,
                    b.ry,
                    t.num("droplet.tip", 1.4, 1.65),
                ));
            }
            Self::Sun => {
                let count = t.int("sun.n", 6, 9);
                let dist = b.rx * t.num("sun.dist", 1.0, 1.08);
                let pr = b.rx * t.num("sun.r", 0.2, 0.26);
                let off = t.num("sun.rot", 0.0, 2.0 * std::f64::consts::PI);
                for i in 0..count {
                    let a = off + 2.0 * std::f64::consts::PI * i as f64 / count as f64;
                    out.petals.push(Circle {
                        cx: b.cx + a.cos() * dist,
                        cy: b.cy + a.sin() * dist,
                        r: pr,
                    });
                }
            }
            _ => {}
        }
    }

    /// The core path. `None` is a superellipse — the eyes are superellipses
    /// too, so it is in every bundle already.
    fn path(&self, b: &Body) -> Path2D {
        match self {
            Self::Organic | Self::Cloud => blob_path(b.cx, b.cy, b.rx, b.ry, &b.radii, b.rot),
            Self::Hexagon | Self::Triangle => {
                let sides = b.sides.unwrap_or(6);
                let round = b.round.unwrap_or(0.3);
                polygon(b.cx, b.cy, b.rx, b.ry, sides, round, b.rot)
            }
            Self::Capsule => box_path(b.cx, b.cy, b.rx - b.ry, b.ry),
            _ => superellipse(b.cx, b.cy, b.rx, b.ry, b.n, b.rot),
        }
    }
}

/// Weighted rather than uniform: round and organic are the everyday shapes,
/// while the louder silhouettes stay finds.
const BANDS: [(Silhouette, f64); 10] = [
    (Silhouette::Round, 0.22),
    (Silhouette::Organic, 0.48),
    (Silhouette::Boxy, 0.6),
    (Silhouette::Capsule, 0.7),
    (Silhouette::Nub, 0.79),
    (Silhouette::Cloud, 0.86),
    (Silhouette::Droplet, 0.915),
    (Silhouette::Hexagon, 0.95),
    (Silhouette::Sun, 0.98),
    (Silhouette::Triangle, 1.0),
];

fn pick(v: f64) -> Silhouette {
    BANDS
        .iter()
        .find(|(_, up_to)| v < *up_to)
        .map(|(s, _)| *s)
        .unwrap_or(Silhouette::Triangle)
}

/// Fits the eye cluster against the silhouette's face region on both axes.
fn eye_fit(t: &Reader, b: &Body, face: Ellipse) -> Vec<Path2D> {
    let rx = b.rx;
    let er0 = t.num("eye.rx", 0.075, 0.105) * rx;
    let ratio = t.num("eye.ratio", 1.9, 3.2);
    let scale = t.num("eye.scale", 0.78, 1.24);
    let stretch = t.num("eye.stretch", 0.85, 1.18);
    let clearance = t.num("eye.gap", 0.1, 0.24) * rx;
    let wide = er0 * scale.max(1.0);
    let tall = er0 * ratio * (scale * stretch).max(1.0);
    let gap0 = wide + rx * 0.03 + clearance;

    let gx = t.jitter("gaze.x", 0.09) * face.rx;
    let gy = t.num("gaze.y", -0.2, 0.08) * face.ry;
    let dy = t.jitter("eye.dy", 0.04) * face.ry;
    let reach = (wide * wide + tall * tall).sqrt();
    let need = (((gx.abs() + gap0 + reach) / face.rx).powi(2)
        + ((gy.abs() + dy.abs() + reach) / face.ry).powi(2))
    .sqrt();
    let fit = if need > 0.9 { 0.9 / need } else { 1.0 };

    let er = er0 * fit;
    let eye_ry = er * ratio;
    let gap = gap0 * fit;
    let room = (clearance / tall).clamp(0.0, 1.0);
    let bound = (room.asin() * 180.0 / std::f64::consts::PI).min(12.0);
    let lean = t.num("eye.lean", -1.0, 1.0) * bound;
    let lean2 = (lean + t.jitter("eye.lean2", 3.5)).clamp(-12.0, 12.0);

    let cx = face.cx + gx * fit;
    let cy = face.cy + gy * fit;
    let n = t.num("eye.n", 3.5, 6.0);
    vec![
        superellipse(cx - gap, cy, er, eye_ry, n, lean),
        superellipse(
            cx + gap,
            cy + dy * fit,
            er * scale,
            eye_ry * scale * stretch,
            n,
            lean2,
        ),
    ]
}

/// One composed blobatar, in the 100×100 design space: everything `avatar()`
/// needs to paint, plus the palette in OKLCh.
#[derive(Clone, Debug, PartialEq)]
pub struct Art {
    pub shape: &'static str,
    pub head: Oklch,
    pub eye: Oklch,
    pub petals: Vec<Circle>,
    pub extra: Vec<Path2D>,
    pub body: Path2D,
    pub eyes: Vec<Path2D>,
}

impl Art {
    pub fn from_name(name: &str) -> Self {
        let t = Reader::new(name);
        let hue = t.num("hue", 0.0, 360.0);
        let tone = t.raw("tone");
        let (head, eye) = super::color::ramp_for(hue, tone);

        let shape = pick(t.raw("shape"));
        let r = t.num("body.r", 31.0, 38.0) * shape.core();
        let mut body = Body {
            cx: 50.0 + t.jitter("body.x", 1.5),
            cy: 50.0 + t.jitter("body.y", 1.5),
            rx: r,
            ry: r * t.num("body.ratio", 0.92, 1.08),
            n: t.num("body.n", 1.9, 2.5),
            rot: 0.0,
            radii: (0..t.int("body.pts", 6, 8))
                .map(|i| 1.0 + t.jitter(&format!("body.r{i}"), 0.16))
                .collect(),
            sides: None,
            round: None,
        };
        shape.body_patch(&t, &mut body);

        let face = shape.face(&body).unwrap_or(Ellipse {
            cx: body.cx,
            cy: body.cy,
            rx: body.rx,
            ry: body.ry,
        });
        let mut deco = Deco::default();
        shape.decorate(&t, &body, &mut deco);

        Self {
            shape: shape.name(),
            head,
            eye,
            petals: deco.petals,
            extra: deco.extra,
            body: shape.path(&body),
            eyes: eye_fit(&t, &body, face),
        }
    }
}
