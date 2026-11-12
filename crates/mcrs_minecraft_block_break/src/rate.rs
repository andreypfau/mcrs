use crate::Miner;

const BLOCK_BREAK_SPEED: f32 = 1.0;
const SUBMERGED_MINING_SPEED: f32 = 0.2;

/// The progress that one tick adds to a dig on a block state with this hardness.
///
/// A rate of 1 or more breaks the block on the start action. A hardness of -1 gives 0, and a
/// hardness of 0 gives 1.
#[must_use]
pub fn rate(hardness: f32, miner: &Miner) -> f32 {
    if hardness == -1.0 {
        return 0.0;
    }
    // A finite 1 and no division: a dig on air then shows the stage 10.
    if hardness == 0.0 {
        return 1.0;
    }
    let mut speed = miner.item_speed * BLOCK_BREAK_SPEED;
    if miner.eyes_in_water {
        speed *= SUBMERGED_MINING_SPEED;
    }
    if !miner.on_ground {
        speed /= 5.0;
    }
    let harvest = if miner.harvests { 30.0 } else { 100.0 };
    speed / hardness / harvest
}
