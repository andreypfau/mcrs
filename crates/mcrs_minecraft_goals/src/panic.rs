use glam::DVec3;
use mcrs_minecraft_brain::DamageCause;
use mcrs_minecraft_brain::MobFlags;
use mcrs_minecraft_brain::MobView;
use mcrs_minecraft_brain::Turn;
use mcrs_minecraft_goal_world::CellFlags;
use mcrs_minecraft_goal_world::GoalWorld;
use mcrs_voxel::BlockPos;

use crate::PANIC_SEARCH;
use crate::SearchRange;
use crate::random_position;

pub const PANIC_FRESH_TICKS: u64 = 40;

const PANIC_SEARCH_RANGE: SearchRange = SearchRange::new(5, 4);
const WATER_REACH: i32 = 5;

#[must_use]
pub fn panic_record_fresh(view: &MobView, tick: u64) -> bool {
    view.damage.is_some_and(|record| {
        record.cause == DamageCause::Panic && tick.saturating_sub(record.tick) <= PANIC_FRESH_TICKS
    })
}

pub fn panic_destination<W: GoalWorld + ?Sized>(turn: &Turn<'_, W>) -> Option<DVec3> {
    let view = turn.view;
    let block = BlockPos::from(view.position);
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
