//! Charts drawn to the width the host has. Pele's
//! `diagrams/common/fit-width.ts`.

use std::convert::Infallible;

use crate::Rendered;
use crate::{Ctx, Host};

const BREAKPOINT: f64 = 640.0;

impl<H: Host> Ctx<'_, H> {
    fn available(&self) -> Option<f64> {
        self.options.max_width.filter(|&w| w > 0.0)
    }

    /// Draws a chart whose plot can be any width so the whole diagram fits
    /// the host's width. What surrounds the plot is only known once it is
    /// drawn, so the plot gives up the overflow and is drawn again. `narrow`
    /// tells the chart it did not fit at its own width.
    pub fn fit_width(
        &self,
        natural: f64,
        least: f64,
        mut draw: impl FnMut(f64, bool) -> Rendered,
    ) -> Rendered {
        match self.try_fit_width(natural, least, |plot, narrow| {
            Ok::<_, Infallible>(draw(plot, narrow))
        }) {
            Ok(result) => result,
            Err(never) => match never {},
        }
    }

    /// [`Ctx::fit_width`] for a chart whose drawing can fail: the first
    /// failure is the answer.
    pub fn try_fit_width<E>(
        &self,
        natural: f64,
        least: f64,
        mut draw: impl FnMut(f64, bool) -> Result<Rendered, E>,
    ) -> Result<Rendered, E> {
        let mut result = draw(natural, false)?;
        let Some(available) = self.available() else {
            return Ok(result);
        };
        if result.width <= available {
            return Ok(result);
        }
        let floor = least.min(natural);
        let mut plot = natural;
        result = draw(plot, true)?;
        let mut round = 0;
        while round < 3 && result.width > available && plot > floor {
            plot = floor.max(plot - (result.width - available));
            result = draw(plot, true)?;
            round += 1;
        }
        Ok(result)
    }

    /// The width a title may take before it wraps.
    pub fn title_room(&self) -> f64 {
        match self.available() {
            Some(available) => 80f64.max(available - 2.0 * self.options.padding.unwrap_or(8.0)),
            None => 4000.0,
        }
    }

    /// For a chart drawn to a set width: its own, or the host's when smaller.
    /// `around` is what the chart adds outside that width.
    pub fn cap_width(&self, natural: f64, least: f64, around: f64) -> f64 {
        match self.available() {
            None => natural,
            Some(available) => least.min(natural).max(natural.min(available - around)),
        }
    }

    /// For a diagram that can run across or down: drawn across, and down
    /// instead when the host allows it, its width is under the breakpoint,
    /// across does not fit, and down is narrower.
    pub fn turn_to_fit(&self, mut draw: impl FnMut(bool) -> Rendered) -> Rendered {
        let across = draw(false);
        let Some(available) = self.available() else {
            return across;
        };
        if !self.options.auto_direction || across.width <= available {
            return across;
        }
        let breakpoint = Some(self.options.direction_breakpoint)
            .filter(|&b| b > 0.0)
            .unwrap_or(BREAKPOINT);
        if available >= breakpoint {
            return across;
        }
        let down = draw(true);
        if down.width < across.width {
            down
        } else {
            across
        }
    }

    /// Draws a diagram with room to spare, and tightly instead when that does
    /// not fit the host's width and tight is narrower.
    pub fn tighten(&self, mut draw: impl FnMut(bool) -> Rendered) -> Rendered {
        match self.try_tighten(|tight| Ok::<_, Infallible>(draw(tight))) {
            Ok(result) => result,
            Err(never) => match never {},
        }
    }

    /// [`Ctx::tighten`] for a diagram whose drawing can fail: the first
    /// failure is the answer.
    pub fn try_tighten<E>(
        &self,
        mut draw: impl FnMut(bool) -> Result<Rendered, E>,
    ) -> Result<Rendered, E> {
        let loose = draw(false)?;
        let Some(available) = self.available() else {
            return Ok(loose);
        };
        if loose.width <= available {
            return Ok(loose);
        }
        let tight = draw(true)?;
        Ok(if tight.width < loose.width {
            tight
        } else {
            loose
        })
    }
}
