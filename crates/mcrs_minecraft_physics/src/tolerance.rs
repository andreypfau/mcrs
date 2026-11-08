/// The tolerance of the collision algorithm, the short-move drop, the block scan and the stuck
/// multiplier.
pub(crate) const COLLISION: f64 = 1.0e-7;

/// `1.0E-5F`, widened: the clip test of the horizontal flag, the downward reach of a step-up and
/// the shrink of an intersection test.
pub(crate) const CLIP: f64 = 1.0e-5_f32 as f64;

/// The depth under the box in which the move finds the supporting block.
pub(crate) const SUPPORT_DEPTH: f64 = 1.0e-6;
