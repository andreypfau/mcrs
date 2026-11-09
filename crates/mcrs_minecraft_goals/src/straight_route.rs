use glam::DVec3;
use mcrs_voxel::BlockPos;

use crate::GoalWorld;

const STEP: f64 = 0.5;
const LONGEST: f64 = 64.0;

/// A straight route on walkable ground: from the block of `from` to `target`, each half block of
/// the line must be walkable at the level of the last sample or one block above or below it. The
/// end is the bottom centre of the target column at the level that the line reaches.
pub fn straight_route<W: GoalWorld + ?Sized>(
    world: &W,
    from: DVec3,
    target: BlockPos,
) -> Option<DVec3> {
    let end = target.bottom_centre();
    let dx = end.x - from.x;
    let dz = end.z - from.z;
    let length = libm::sqrt(dx * dx + dz * dz);
    if length > LONGEST {
        return None;
    }
    let steps = libm::ceil(length / STEP) as u32;
    let mut level = BlockPos::from(from).y;
    for step in 1..=steps {
        let share = f64::from(step) / f64::from(steps);
        let sample = BlockPos::from(DVec3::new(from.x + dx * share, 0.0, from.z + dz * share));
        level = [0, 1, -1].into_iter().map(|rise| level + rise).find(|y| {
            world
                .cell(BlockPos::new(sample.x, *y, sample.z))
                .is_walkable()
        })?;
    }
    if steps == 0
        && !world
            .cell(BlockPos::new(target.x, level, target.z))
            .is_walkable()
    {
        return None;
    }
    Some(end.with_y(f64::from(level)))
}
