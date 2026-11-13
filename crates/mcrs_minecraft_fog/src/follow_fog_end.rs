const FOLLOW_RATE_PER_TICK: f32 = 0.1;
/// Blocks. Without it the end stops a few ulp short of a target it moves toward.
const SNAP: f32 = 0.01;

/// The end of the render-distance fog of a frame, in blocks, from the end of the previous frame,
/// the `target` of [`DrawnRings::fog_end_target`](crate::DrawnRings::fog_end_target) and the time
/// since the previous frame in ticks. The end drops to a nearer target at once and moves toward
/// a farther one, which it reaches.
#[must_use]
pub fn follow_fog_end(previous: f32, target: f32, ticks: f32) -> f32 {
    if target <= previous {
        return target;
    }
    let end = previous + (target - previous) * (FOLLOW_RATE_PER_TICK * ticks).min(1.0);
    if target - end < SNAP { target } else { end }
}
