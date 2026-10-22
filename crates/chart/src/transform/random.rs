//! The random numbers transforms draw: vega-statistics' `randomLCG`.

/// The seed every [`Random`] starts from: Vega's `randomLCG(123456789)`.
const SEED: f64 = 123_456_789.0;

/// One generator, which every op of a chart draws from in turn as Vega's
/// global `random` is.
#[derive(Clone, Debug, PartialEq)]
pub struct Random(f64);

impl Default for Random {
    fn default() -> Self {
        Self(SEED)
    }
}

/// Numbers in `[0, 1)`, without end.
impl Iterator for Random {
    type Item = f64;

    fn next(&mut self) -> Option<f64> {
        self.0 = (1_103_515_245.0 * self.0 + 12_345.0) % 2_147_483_647.0;
        Some(self.0 / 2_147_483_647.0)
    }
}
