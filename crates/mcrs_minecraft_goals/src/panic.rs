use glam::DVec3;
use mcrs_minecraft_brain::BlockPos;
use mcrs_minecraft_brain::DamageCause;
use mcrs_minecraft_brain::MobFlags;
use mcrs_minecraft_brain::MobView;
use mcrs_minecraft_brain::Turn;

use crate::CellFlags;
use crate::GoalWorld;
use crate::PANIC_SEARCH;
use crate::SearchRange;
use crate::random_position;

/// The panic goal (GOAL-PNC-002) runs while its path runs; it starts while the damage record is
/// fresh, at most this many ticks after the hit.
pub const PANIC_FRESH_TICKS: u64 = 40;

const PANIC_SEARCH_RANGE: SearchRange = SearchRange::new(5, 4);
const WATER_REACH: i32 = 5;

/// The damage record of the view is fresh at `tick` and its type causes panic.
#[must_use]
pub fn panic_record_fresh(view: &MobView, tick: u64) -> bool {
    view.damage.is_some_and(|record| {
        record.cause == DamageCause::Panic && tick.saturating_sub(record.tick) <= PANIC_FRESH_TICKS
    })
}

/// The destination of a panic: the nearest water of `Q-WATER` for a mob on fire in a block with no
/// collision shape, else the default search `D(5, 4)`.
pub fn panic_destination<W: GoalWorld + ?Sized>(turn: &Turn<'_, W>) -> Option<DVec3> {
    let view = turn.view;
    let block = BlockPos::containing(view.position);
    if view.flags.contains(MobFlags::ON_FIRE)
        && turn
            .world
            .cell(block)
            .flags
            .contains(CellFlags::NO_COLLISION)
        && let Some(water) = nearest_water(turn.world, block)
    {
        return Some(DVec3::new(
            f64::from(water.x),
            f64::from(water.y),
            f64::from(water.z),
        ));
    }
    random_position::default_search(
        turn.world,
        view.position,
        PANIC_SEARCH_RANGE,
        &mut turn.random(PANIC_SEARCH),
    )
}

/// `Q-WATER` (GOAL-PNC-003): the water block with the smallest Manhattan distance within 5
/// blocks horizontally and 1 vertically; on a tie the smaller `dx`, then the smaller `dy`, then a
/// positive `dz` before a negative one.
fn nearest_water<W: GoalWorld + ?Sized>(world: &W, centre: BlockPos) -> Option<BlockPos> {
    let mut best: Option<(WaterRank, BlockPos)> = None;
    for dx in -WATER_REACH..=WATER_REACH {
        for dy in -1..=1 {
            for dz in -WATER_REACH..=WATER_REACH {
                let at = centre.offset(dx, dy, dz);
                if !world.cell(at).flags.contains(CellFlags::WATER) {
                    continue;
                }
                let rank = WaterRank {
                    distance: dx.abs() + dy.abs() + dz.abs(),
                    dx,
                    dy,
                    negative_dz: dz < 0,
                };
                if best.is_none_or(|(best_rank, _)| rank < best_rank) {
                    best = Some((rank, at));
                }
            }
        }
    }
    best.map(|(_, at)| at)
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct WaterRank {
    distance: i32,
    dx: i32,
    dy: i32,
    negative_dz: bool,
}
