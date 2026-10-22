//! Vega's named colour schemes, as vega-scale 8.1.3 registers them, and how
//! Vega's Scale transform fits one to a scale: an interpolator for a
//! sequential or diverging scale, a list of colours for a discrete one.

use crate::{color::Rgb, interpolate::Interpolate};
use std::str::FromStr;

/// What a scheme holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    /// Colours a discrete scale takes in turn.
    Discrete,
    /// Colours spread evenly across `0..=1`, interpolated in RGB between.
    Continuous,
}

/// A named scheme. Names are matched ignoring case.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Scheme {
    pub name: &'static str,
    kind: Kind,
    colors: &'static [u32],
}

impl FromStr for Scheme {
    type Err = ();

    fn from_str(name: &str) -> Result<Self, ()> {
        let name = name.to_lowercase();
        let index = SCHEMES
            .binary_search_by_key(&name.as_str(), |(name, ..)| name)
            .map_err(|_| ())?;
        let (name, kind, colors) = SCHEMES[index];
        Ok(Self { name, kind, colors })
    }
}

impl Scheme {
    /// Whether it is a ramp, which Vega interpolates, rather than a list.
    pub fn is_continuous(&self) -> bool {
        self.kind == Kind::Continuous
    }

    /// Its colours as listed.
    pub fn colors(&self) -> Vec<Rgb> {
        self.colors
            .iter()
            .map(|&n| Rgb {
                r: ((n >> 16) & 0xff) as f64,
                g: ((n >> 8) & 0xff) as f64,
                b: (n & 0xff) as f64,
                opacity: 1.0,
            })
            .collect()
    }

    /// A ramp's colour at `t` in `0..=1`, through `extent` (Vega's
    /// `schemeExtent`, `[0, 1]` for the whole ramp), which `reverse` turns
    /// round. A list's colour at the nearest place.
    pub fn at(&self, t: f64, extent: Option<[f64; 2]>, reverse: bool) -> Rgb {
        let [start, end] = extent.unwrap_or([0.0, 1.0]);
        let (start, end) = match reverse {
            true => (end, start),
            false => (start, end),
        };
        Interpolate::Rgb { gamma: 1.0 }.piecewise(&self.colors(), start + t * (end - start))
    }

    /// The colours a discrete scale of `count` values takes: a list as it
    /// is, cut to `count` unless `ordinal`; a ramp sampled at `count` evenly
    /// spaced places strictly inside `extent`.
    pub fn discrete(&self, count: usize, extent: Option<[f64; 2]>, ordinal: bool) -> Vec<Rgb> {
        match self.kind {
            Kind::Discrete if ordinal => self.colors(),
            Kind::Discrete => self.colors().into_iter().take(count).collect(),
            Kind::Continuous => (1..=count)
                .map(|i| self.at(i as f64 / (count + 1) as f64, extent, false))
                .collect(),
        }
    }
}

const SCHEMES: [(&str, Kind, &[u32]); 68] = [
    (
        "accent",
        Kind::Discrete,
        &[
            0x7fc97f, 0xbeaed4, 0xfdc086, 0xffff99, 0x386cb0, 0xf0027f, 0xbf5b17, 0x666666,
        ],
    ),
    (
        "bluegreen",
        Kind::Continuous,
        &[
            0xd5efed, 0xc1e8e0, 0xa7ddd1, 0x8bd2be, 0x70c6a9, 0x58ba91, 0x44ad77, 0x319c5d,
            0x208946, 0x0e7736, 0x036429,
        ],
    ),
    (
        "blueorange",
        Kind::Continuous,
        &[
            0x134b85, 0x2f78b3, 0x5da2cb, 0x9dcae1, 0xd2e5ef, 0xf2f0eb, 0xfce0ba, 0xfbbf74,
            0xe8932f, 0xc5690d, 0x994a07,
        ],
    ),
    (
        "bluepurple",
        Kind::Continuous,
        &[
            0xccddec, 0xbad0e4, 0xa8c2dd, 0x9ab0d4, 0x919cc9, 0x8d85be, 0x8b6db2, 0x8a55a6,
            0x873c99, 0x822287, 0x730f71,
        ],
    ),
    (
        "blues",
        Kind::Continuous,
        &[
            0xcfe1f2, 0xbed8ec, 0xa8cee5, 0x8fc1de, 0x74b2d7, 0x5ba3cf, 0x4592c6, 0x3181bd,
            0x206fb2, 0x125ca4, 0x0a4a90,
        ],
    ),
    (
        "brownbluegreen",
        Kind::Continuous,
        &[
            0x704108, 0xa0651a, 0xc79548, 0xe3c78a, 0xf3e6c6, 0xeef1ea, 0xc9e9e4, 0x8ed1c7,
            0x4da79e, 0x187a72, 0x025147,
        ],
    ),
    (
        "browns",
        Kind::Continuous,
        &[
            0xeedbbd, 0xecca96, 0xe9b97a, 0xe4a865, 0xdc9856, 0xd18954, 0xc7784c, 0xc0673f,
            0xb85536, 0xad4433, 0x9f3632,
        ],
    ),
    (
        "category10",
        Kind::Discrete,
        &[
            0x1f77b4, 0xff7f0e, 0x2ca02c, 0xd62728, 0x9467bd, 0x8c564b, 0xe377c2, 0x7f7f7f,
            0xbcbd22, 0x17becf,
        ],
    ),
    (
        "category20",
        Kind::Discrete,
        &[
            0x1f77b4, 0xaec7e8, 0xff7f0e, 0xffbb78, 0x2ca02c, 0x98df8a, 0xd62728, 0xff9896,
            0x9467bd, 0xc5b0d5, 0x8c564b, 0xc49c94, 0xe377c2, 0xf7b6d2, 0x7f7f7f, 0xc7c7c7,
            0xbcbd22, 0xdbdb8d, 0x17becf, 0x9edae5,
        ],
    ),
    (
        "category20b",
        Kind::Discrete,
        &[
            0x393b79, 0x5254a3, 0x6b6ecf, 0x9c9ede, 0x637939, 0x8ca252, 0xb5cf6b, 0xcedb9c,
            0x8c6d31, 0xbd9e39, 0xe7ba52, 0xe7cb94, 0x843c39, 0xad494a, 0xd6616b, 0xe7969c,
            0x7b4173, 0xa55194, 0xce6dbd, 0xde9ed6,
        ],
    ),
    (
        "category20c",
        Kind::Discrete,
        &[
            0x3182bd, 0x6baed6, 0x9ecae1, 0xc6dbef, 0xe6550d, 0xfd8d3c, 0xfdae6b, 0xfdd0a2,
            0x31a354, 0x74c476, 0xa1d99b, 0xc7e9c0, 0x756bb1, 0x9e9ac8, 0xbcbddc, 0xdadaeb,
            0x636363, 0x969696, 0xbdbdbd, 0xd9d9d9,
        ],
    ),
    (
        "cividis",
        Kind::Continuous,
        &[
            0x002051, 0x002358, 0x00265d, 0x002961, 0x012b65, 0x042e67, 0x083169, 0x0d346b,
            0x11366c, 0x16396d, 0x1c3c6e, 0x213f6e, 0x26426e, 0x2c456e, 0x31476e, 0x374a6e,
            0x3c4d6e, 0x42506e, 0x47536d, 0x4c566d, 0x51586e, 0x555b6e, 0x5a5e6e, 0x5e616e,
            0x62646f, 0x66676f, 0x6a6a70, 0x6e6d71, 0x727071, 0x757372, 0x797673, 0x7c7974,
            0x7f7c75, 0x827f75, 0x868276, 0x898577, 0x8c8877, 0x908b78, 0x938e78, 0x969178,
            0x9a9478, 0x9e9778, 0xa19b78, 0xa59e77, 0xa9a177, 0xaea575, 0xb2a874, 0xb6ab73,
            0xbbaf71, 0xc0b26f, 0xc5b66d, 0xc9b96a, 0xcebd68, 0xd3c065, 0xd8c462, 0xddc85f,
            0xe2cb5c, 0xe7cf58, 0xebd355, 0xf0d652, 0xf3da4f, 0xf7de4c, 0xfae249, 0xfce647,
        ],
    ),
    (
        "dark2",
        Kind::Discrete,
        &[
            0x1b9e77, 0xd95f02, 0x7570b3, 0xe7298a, 0x66a61e, 0xe6ab02, 0xa6761d, 0x666666,
        ],
    ),
    (
        "darkblue",
        Kind::Continuous,
        &[
            0x323232, 0x2d4668, 0x1a5c93, 0x0074af, 0x008cbf, 0x05a7ce, 0x25c0dd, 0x38daed,
            0x50f3fa, 0xffffff,
        ],
    ),
    (
        "darkgold",
        Kind::Continuous,
        &[
            0x3c3c3c, 0x584b37, 0x725e34, 0x8c7631, 0xae8b2b, 0xcfa424, 0xecc31e, 0xf9de30,
            0xfff184, 0xffffff,
        ],
    ),
    (
        "darkgreen",
        Kind::Continuous,
        &[
            0x3a3a3a, 0x215748, 0x006f4d, 0x048942, 0x489e42, 0x76b340, 0xa6c63d, 0xd2d836,
            0xffeb2c, 0xffffaa,
        ],
    ),
    (
        "darkmulti",
        Kind::Continuous,
        &[
            0x373737, 0x1f5287, 0x197d8c, 0x29a869, 0x95ce3f, 0xffe800, 0xffffff,
        ],
    ),
    (
        "darkred",
        Kind::Continuous,
        &[
            0x343434, 0x703633, 0x9e3c38, 0xcc4037, 0xe75d1e, 0xec8620, 0xeeab29, 0xf0ce32,
            0xffeb2c,
        ],
    ),
    (
        "goldgreen",
        Kind::Continuous,
        &[
            0xf4d166, 0xd5ca60, 0xb6c35c, 0x98bb59, 0x7cb257, 0x60a656, 0x4b9c53, 0x3f8f4f,
            0x33834a, 0x257740, 0x146c36,
        ],
    ),
    (
        "goldorange",
        Kind::Continuous,
        &[
            0xf4d166, 0xf8be5c, 0xf8aa4c, 0xf5983b, 0xf3852a, 0xef701b, 0xe2621f, 0xd65322,
            0xc54923, 0xb14223, 0x9e3a26,
        ],
    ),
    (
        "goldred",
        Kind::Continuous,
        &[
            0xf4d166, 0xf6be59, 0xf9aa51, 0xfc964e, 0xf6834b, 0xee734a, 0xe56249, 0xdb5247,
            0xcf4244, 0xc43141, 0xb71d3e,
        ],
    ),
    (
        "greenblue",
        Kind::Continuous,
        &[
            0xd3eece, 0xc5e8c3, 0xb1e1bb, 0x9bd8bb, 0x82cec2, 0x69c2ca, 0x51b2cd, 0x3c9fc7,
            0x288abd, 0x1675b1, 0x0b60a1,
        ],
    ),
    (
        "greens",
        Kind::Continuous,
        &[
            0xd3eecd, 0xc0e6ba, 0xabdda5, 0x94d391, 0x7bc77d, 0x60ba6c, 0x46ab5e, 0x329a51,
            0x208943, 0x0e7735, 0x036429,
        ],
    ),
    (
        "greys",
        Kind::Continuous,
        &[
            0xe2e2e2, 0xd4d4d4, 0xc4c4c4, 0xb1b1b1, 0x9d9d9d, 0x888888, 0x757575, 0x626262,
            0x4d4d4d, 0x353535, 0x1e1e1e,
        ],
    ),
    (
        "inferno",
        Kind::Continuous,
        &[
            0x000004, 0x040313, 0x0c0826, 0x170c3b, 0x240c4f, 0x330a5f, 0x420a68, 0x500d6c,
            0x5d126e, 0x6b176e, 0x781c6d, 0x86216b, 0x932667, 0xa12b62, 0xae305c, 0xbb3755,
            0xc73e4c, 0xd24644, 0xdd513a, 0xe65c30, 0xed6925, 0xf3771a, 0xf8850f, 0xfb9506,
            0xfca50a, 0xfcb519, 0xfac62d, 0xf6d645, 0xf2e661, 0xf3f484, 0xfcffa4,
        ],
    ),
    (
        "lightgreyred",
        Kind::Continuous,
        &[
            0xefe9e6, 0xe1dad7, 0xd5cbc8, 0xc8bdb9, 0xbbaea9, 0xcd967d, 0xdc7b43, 0xe15f19,
            0xdf4011, 0xdc000b,
        ],
    ),
    (
        "lightgreyteal",
        Kind::Continuous,
        &[
            0xe4eaea, 0xd6dcdd, 0xc8ced2, 0xb7c2c7, 0xa6b4bc, 0x64b0bf, 0x22a6c3, 0x2295c1,
            0x1f85be, 0x1876bc,
        ],
    ),
    (
        "lightmulti",
        Kind::Continuous,
        &[
            0xe0f1f2, 0xc4e9d0, 0xb0de9f, 0xd0e181, 0xf6e072, 0xf6c053, 0xf3993e, 0xf77440,
            0xef4a3c,
        ],
    ),
    (
        "lightorange",
        Kind::Continuous,
        &[
            0xf2e7da, 0xf7d5ba, 0xf9c499, 0xfab184, 0xfa9c73, 0xf68967, 0xef7860, 0xe8645b,
            0xde515b, 0xd43d5b,
        ],
    ),
    (
        "lighttealblue",
        Kind::Continuous,
        &[
            0xe3e9e0, 0xc0dccf, 0x9aceca, 0x7abfc8, 0x59afc0, 0x389fb9, 0x328dad, 0x2f7ca0,
            0x276b95, 0x255988,
        ],
    ),
    (
        "magma",
        Kind::Continuous,
        &[
            0x000004, 0x040413, 0x0b0924, 0x150e37, 0x20114b, 0x2c1160, 0x3b0f70, 0x4a1079,
            0x57157e, 0x651a80, 0x721f81, 0x7f2482, 0x8c2981, 0x9a2e80, 0xa8327d, 0xb6377a,
            0xc43c75, 0xd1426f, 0xde4968, 0xe95462, 0xf1605d, 0xf76f5c, 0xfa7f5e, 0xfc8f65,
            0xfe9f6d, 0xfeaf78, 0xfebf84, 0xfece91, 0xfddea0, 0xfcedaf, 0xfcfdbf,
        ],
    ),
    (
        "observable10",
        Kind::Discrete,
        &[
            0x4269d0, 0xefb118, 0xff725c, 0x6cc5b0, 0x3ca951, 0xff8ab7, 0xa463f2, 0x97bbf5,
            0x9c6b4e, 0x9498a0,
        ],
    ),
    (
        "orangered",
        Kind::Continuous,
        &[
            0xfddcaf, 0xfdcf9b, 0xfdc18a, 0xfdad77, 0xfb9562, 0xf67d53, 0xee6545, 0xe24932,
            0xd32d1e, 0xbf130d, 0xa70403,
        ],
    ),
    (
        "oranges",
        Kind::Continuous,
        &[
            0xfdd8b3, 0xfdc998, 0xfdb87b, 0xfda55e, 0xfc9244, 0xf87f2c, 0xf06b18, 0xe4580b,
            0xd14904, 0xb93d02, 0x9f3303,
        ],
    ),
    (
        "paired",
        Kind::Discrete,
        &[
            0xa6cee3, 0x1f78b4, 0xb2df8a, 0x33a02c, 0xfb9a99, 0xe31a1c, 0xfdbf6f, 0xff7f00,
            0xcab2d6, 0x6a3d9a, 0xffff99, 0xb15928,
        ],
    ),
    (
        "pastel1",
        Kind::Discrete,
        &[
            0xfbb4ae, 0xb3cde3, 0xccebc5, 0xdecbe4, 0xfed9a6, 0xffffcc, 0xe5d8bd, 0xfddaec,
            0xf2f2f2,
        ],
    ),
    (
        "pastel2",
        Kind::Discrete,
        &[
            0xb3e2cd, 0xfdcdac, 0xcbd5e8, 0xf4cae4, 0xe6f5c9, 0xfff2ae, 0xf1e2cc, 0xcccccc,
        ],
    ),
    (
        "pinkyellowgreen",
        Kind::Continuous,
        &[
            0x8e0152, 0xc0267e, 0xdd72ad, 0xf0b3d6, 0xfadded, 0xf5f3ef, 0xe1f2ca, 0xb6de87,
            0x80bb47, 0x4f9125, 0x276419,
        ],
    ),
    (
        "plasma",
        Kind::Continuous,
        &[
            0x0d0887, 0x230690, 0x330597, 0x42039d, 0x5002a2, 0x5d01a6, 0x6a00a8, 0x7801a8,
            0x8405a7, 0x900da4, 0x9c179e, 0xa72198, 0xb12a90, 0xba3488, 0xc33d80, 0xcb4779,
            0xd35171, 0xda5a69, 0xe16462, 0xe76e5b, 0xed7953, 0xf2834c, 0xf68f44, 0xfa9a3d,
            0xfca636, 0xfdb32f, 0xfec029, 0xfcce25, 0xf9dc24, 0xf5ea27, 0xf0f921,
        ],
    ),
    (
        "purpleblue",
        Kind::Continuous,
        &[
            0xdbdaeb, 0xc8cee4, 0xb1c3de, 0x97b7d8, 0x7bacd1, 0x5b9fc9, 0x3a90c0, 0x1e7fb7,
            0x0b70ab, 0x056199, 0x045281,
        ],
    ),
    (
        "purplebluegreen",
        Kind::Continuous,
        &[
            0xdbd8ea, 0xc8cee4, 0xb0c3de, 0x93b7d8, 0x72acd1, 0x549fc8, 0x3892bb, 0x1c88a3,
            0x097f87, 0x02736b, 0x016353,
        ],
    ),
    (
        "purplegreen",
        Kind::Continuous,
        &[
            0x5b1667, 0x834792, 0xa67fb6, 0xc9aed3, 0xe6d6e8, 0xeff0ef, 0xd9efd5, 0xaedda9,
            0x71bb75, 0x368e49, 0x0e5e29,
        ],
    ),
    (
        "purpleorange",
        Kind::Continuous,
        &[
            0x411469, 0x664796, 0x8f83b7, 0xb9b4d6, 0xdadbeb, 0xf3eeea, 0xfce0ba, 0xfbbf74,
            0xe8932f, 0xc5690d, 0x994a07,
        ],
    ),
    (
        "purplered",
        Kind::Continuous,
        &[
            0xdcc9e2, 0xd3b3d7, 0xce9ecc, 0xd186c0, 0xda6bb2, 0xe14da0, 0xe23189, 0xd91e6f,
            0xc61159, 0xab0749, 0x8f023a,
        ],
    ),
    (
        "purples",
        Kind::Continuous,
        &[
            0xe2e1ef, 0xd4d4e8, 0xc4c5e0, 0xb4b3d6, 0xa3a0cc, 0x928ec3, 0x827cb9, 0x7566ae,
            0x684ea2, 0x5c3696, 0x501f8c,
        ],
    ),
    (
        "rainbow",
        Kind::Continuous,
        &[
            0x6e40aa, 0x883eb1, 0xa43db3, 0xbf3caf, 0xd83fa4, 0xee4395, 0xfe4b83, 0xff576e,
            0xff6659, 0xff7847, 0xff8c38, 0xf3a130, 0xe2b72f, 0xcfcc36, 0xbee044, 0xaff05b,
            0x8ff457, 0x6ff65b, 0x52f667, 0x3af278, 0x28ea8d, 0x1ddfa3, 0x19d0b8, 0x1cbecb,
            0x23abd8, 0x2f96e0, 0x3d82e1, 0x4c6edb, 0x5a5dd0, 0x664dbf, 0x6e40aa,
        ],
    ),
    (
        "redblue",
        Kind::Continuous,
        &[
            0x8c0d25, 0xbf363a, 0xdf745e, 0xf4ae91, 0xfbdbc9, 0xf2efee, 0xd2e5ef, 0x9dcae1,
            0x5da2cb, 0x2f78b3, 0x134b85,
        ],
    ),
    (
        "redgrey",
        Kind::Continuous,
        &[
            0x8c0d25, 0xbf363a, 0xdf745e, 0xf4ae91, 0xfcdccb, 0xfaf4f1, 0xe2e2e2, 0xc0c0c0,
            0x969696, 0x646464, 0x343434,
        ],
    ),
    (
        "redpurple",
        Kind::Continuous,
        &[
            0xfccfcc, 0xfcbec0, 0xfaa9b8, 0xf98faf, 0xf571a5, 0xec539d, 0xdb3695, 0xc41b8a,
            0xa90880, 0x8d0179, 0x700174,
        ],
    ),
    (
        "reds",
        Kind::Continuous,
        &[
            0xfdc9b4, 0xfcb49a, 0xfc9e80, 0xfc8767, 0xfa7051, 0xf6573f, 0xec3f2f, 0xdc2a25,
            0xc81b1d, 0xb21218, 0x970b13,
        ],
    ),
    (
        "redyellowblue",
        Kind::Continuous,
        &[
            0xa50026, 0xd4322c, 0xf16e43, 0xfcac64, 0xfedd90, 0xfaf8c1, 0xdcf1ec, 0xabd6e8,
            0x75abd0, 0x4a74b4, 0x313695,
        ],
    ),
    (
        "redyellowgreen",
        Kind::Continuous,
        &[
            0xa50026, 0xd4322c, 0xf16e43, 0xfcac63, 0xfedd8d, 0xf9f7ae, 0xd7ee8e, 0xa4d86e,
            0x64bc61, 0x22964f, 0x006837,
        ],
    ),
    (
        "set1",
        Kind::Discrete,
        &[
            0xe41a1c, 0x377eb8, 0x4daf4a, 0x984ea3, 0xff7f00, 0xffff33, 0xa65628, 0xf781bf,
            0x999999,
        ],
    ),
    (
        "set2",
        Kind::Discrete,
        &[
            0x66c2a5, 0xfc8d62, 0x8da0cb, 0xe78ac3, 0xa6d854, 0xffd92f, 0xe5c494, 0xb3b3b3,
        ],
    ),
    (
        "set3",
        Kind::Discrete,
        &[
            0x8dd3c7, 0xffffb3, 0xbebada, 0xfb8072, 0x80b1d3, 0xfdb462, 0xb3de69, 0xfccde5,
            0xd9d9d9, 0xbc80bd, 0xccebc5, 0xffed6f,
        ],
    ),
    (
        "sinebow",
        Kind::Continuous,
        &[
            0xff4040, 0xfc582a, 0xf47218, 0xe78d0b, 0xd5a703, 0xbfbf00, 0xa7d503, 0x8de70b,
            0x72f418, 0x58fc2a, 0x40ff40, 0x2afc58, 0x18f472, 0x0be78d, 0x03d5a7, 0x00bfbf,
            0x03a7d5, 0x0b8de7, 0x1872f4, 0x2a58fc, 0x4040ff, 0x582afc, 0x7218f4, 0x8d0be7,
            0xa703d5, 0xbf00bf, 0xd503a7, 0xe70b8d, 0xf41872, 0xfc2a58, 0xff4040,
        ],
    ),
    (
        "spectral",
        Kind::Continuous,
        &[
            0x9e0142, 0xd13c4b, 0xf0704a, 0xfcac63, 0xfedd8d, 0xfbf8b0, 0xe0f3a1, 0xa9dda2,
            0x69bda9, 0x4288b5, 0x5e4fa2,
        ],
    ),
    (
        "tableau10",
        Kind::Discrete,
        &[
            0x4c78a8, 0xf58518, 0xe45756, 0x72b7b2, 0x54a24b, 0xeeca3b, 0xb279a2, 0xff9da6,
            0x9d755d, 0xbab0ac,
        ],
    ),
    (
        "tableau20",
        Kind::Discrete,
        &[
            0x4c78a8, 0x9ecae9, 0xf58518, 0xffbf79, 0x54a24b, 0x88d27a, 0xb79a20, 0xf2cf5b,
            0x439894, 0x83bcb6, 0xe45756, 0xff9d98, 0x79706e, 0xbab0ac, 0xd67195, 0xfcbfd2,
            0xb279a2, 0xd6a5c9, 0x9e765f, 0xd8b5a5,
        ],
    ),
    (
        "tealblues",
        Kind::Continuous,
        &[
            0xbce4d8, 0x9dd3d1, 0x81c3cb, 0x65b3c2, 0x45a2b9, 0x368fae, 0x347da0, 0x306a93,
            0x2c5985,
        ],
    ),
    (
        "teals",
        Kind::Continuous,
        &[
            0xbbdfdf, 0xa2d4d5, 0x8ac9c9, 0x75bcbb, 0x61b0af, 0x4da5a4, 0x379998, 0x2b8b8c,
            0x1e7f7f, 0x127273, 0x006667,
        ],
    ),
    (
        "turbo",
        Kind::Continuous,
        &[
            0x23171b, 0x32204a, 0x3e2a71, 0x453493, 0x493eae, 0x4b49c5, 0x4a53d7, 0x485ee4,
            0x4569ee, 0x4074f5, 0x3c7ff8, 0x378af9, 0x3295f7, 0x2e9ff4, 0x2ba9ef, 0x28b3e9,
            0x26bce1, 0x25c5d9, 0x25cdcf, 0x27d5c6, 0x29dcbc, 0x2de3b2, 0x32e9a7, 0x38ee9d,
            0x3ff393, 0x47f689, 0x50f980, 0x5afc77, 0x65fd6e, 0x70fe66, 0x7cfd5e, 0x88fc57,
            0x95fb51, 0xa1f84b, 0xadf545, 0xb9f140, 0xc5ec3c, 0xd0e637, 0xdae034, 0xe4d931,
            0xecd12e, 0xf4c92b, 0xfac029, 0xffb626, 0xffad24, 0xffa223, 0xff9821, 0xff8d1f,
            0xff821d, 0xff771c, 0xfd6c1a, 0xf76118, 0xf05616, 0xe84b14, 0xdf4111, 0xd5380f,
            0xcb2f0d, 0xc0260a, 0xb61f07, 0xac1805, 0xa31302, 0x9b0f00, 0x950c00, 0x910b00,
        ],
    ),
    (
        "viridis",
        Kind::Continuous,
        &[
            0x440154, 0x470e61, 0x481a6c, 0x482575, 0x472f7d, 0x443a83, 0x414487, 0x3d4e8a,
            0x39568c, 0x35608d, 0x31688e, 0x2d708e, 0x2a788e, 0x27818e, 0x23888e, 0x21918d,
            0x1f988b, 0x1fa088, 0x22a884, 0x2ab07f, 0x35b779, 0x43bf71, 0x54c568, 0x66cc5d,
            0x7ad151, 0x8fd744, 0xa5db36, 0xbcdf27, 0xd2e21b, 0xe9e51a, 0xfde725,
        ],
    ),
    (
        "warmgreys",
        Kind::Continuous,
        &[
            0xdcd4d0, 0xcec5c1, 0xc0b8b4, 0xb3aaa7, 0xa59c99, 0x98908c, 0x8b827f, 0x7e7673,
            0x726866, 0x665c5a, 0x59504e,
        ],
    ),
    (
        "yellowgreen",
        Kind::Continuous,
        &[
            0xe4f4ac, 0xd1eca0, 0xb9e294, 0x9ed688, 0x80c97c, 0x62bb6e, 0x47aa5e, 0x329750,
            0x208344, 0x0e723b, 0x036034,
        ],
    ),
    (
        "yellowgreenblue",
        Kind::Continuous,
        &[
            0xeff9bd, 0xdbf1b4, 0xbde5b5, 0x94d5b9, 0x69c5be, 0x45b4c2, 0x2c9ec0, 0x2182b8,
            0x2163aa, 0x23479c, 0x1c3185,
        ],
    ),
    (
        "yelloworangebrown",
        Kind::Continuous,
        &[
            0xfeeaa1, 0xfedd84, 0xfecc63, 0xfeb746, 0xfca031, 0xf68921, 0xeb7215, 0xdb5e0b,
            0xc54c05, 0xab3d03, 0x8f3204,
        ],
    ),
    (
        "yelloworangered",
        Kind::Continuous,
        &[
            0xfee087, 0xfed16f, 0xfebd59, 0xfea849, 0xfd903e, 0xfc7335, 0xf9522b, 0xee3423,
            0xde1b20, 0xca0b22, 0xaf0225,
        ],
    ),
];
