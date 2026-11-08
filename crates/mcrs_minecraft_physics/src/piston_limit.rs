use glam::DVec3;

use crate::Axis;
use crate::tolerance::CLIP;
use crate::tolerance::COLLISION;

const LIMIT: f64 = 0.51;

/// The piston displacement of an entity on each axis in one game tick.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PistonSums {
    pub tick: u64,
    pub sums: DVec3,
}

/// Limits the displacement of a move of the kind `piston`: one axis, and at most 0.51 on each
/// axis in one game tick. `None` stops the move.
pub fn limit_piston(d: DVec3, tick: u64, sums: &mut PistonSums) -> Option<DVec3> {
    if d.length_squared() <= COLLISION {
        return Some(d);
    }
    let axis = Axis::ALL.into_iter().find(|axis| axis.of(d) != 0.0)?;
    if sums.tick != tick {
        *sums = PistonSums {
            tick,
            sums: DVec3::ZERO,
        };
    }
    let old = axis.of(sums.sums);
    let clamped = (old + axis.of(d)).clamp(-LIMIT, LIMIT);
    let component = clamped - old;
    sums.sums = axis.with(sums.sums, clamped);
    (component.abs() > CLIP).then(|| axis.with(DVec3::ZERO, component))
}
