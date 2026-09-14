use bevy_math::IVec3;
use mcrs_minecraft_random::Random;
use mcrs_minecraft_random::xoroshiro::XoroshiroRandom;
use mcrs_minecraft_worldgen::feature::placer::{Predicate, StateMask, WorldGenVolume};
use mcrs_minecraft_worldgen::value_provider::IntProvider;

use crate::feature::tree::provider::StateProvider;
use mcrs_voxel_math::{BlockPos, dist_manhattan};

/// One `stepped_column_cluster` feature with every name it carries already
/// resolved.
#[derive(Clone, Debug)]
pub struct CompiledSteppedColumnCluster {
    pub block: StateProvider,
    pub continue_through: Predicate,
    pub can_replace: Predicate,
    pub cannot_place_on: StateMask,
    pub cluster_reach: IntProvider,
    pub column_count: IntProvider,
    pub column_reach: IntProvider,
    pub height: IntProvider,
}

/// `SteppedColumnClusterFeature.place`: a count of columns scattered over a
/// square around the origin, each stepping down as it leaves its own centre.
///
/// The scatter is drawn lazily, three bounded ints per column and the column's
/// own reach right after, so a column outside the cluster's height still costs
/// its position.
pub fn place_stepped_column_cluster<W>(
    config: &CompiledSteppedColumnCluster,
    volume: &mut W,
    rng: &mut XoroshiroRandom,
    origin: BlockPos,
) -> bool
where
    W: WorldGenVolume,
{
    if !can_place_at(config, volume, origin) {
        return false;
    }
    let column_height = config.height.sample(rng);
    let cluster_reach = column_height.min(config.cluster_reach.sample(rng));
    let count = config.column_count.sample(rng);
    let span = 2 * cluster_reach + 1;

    let mut placed = false;
    for _ in 0..count {
        let x = origin.x - cluster_reach + rng.next_i32_bound(span);
        let y = origin.y + rng.next_i32_bound(1);
        let z = origin.z - cluster_reach + rng.next_i32_bound(span);
        let center = BlockPos::new(x, y, z);
        let height = column_height - dist_manhattan(center.as_ivec3(), origin.as_ivec3());
        if height >= 0 {
            let reach = config.column_reach.sample(rng);
            placed |= place_column(config, volume, rng, center, height, reach);
        }
    }
    placed
}

fn place_column<W>(
    config: &CompiledSteppedColumnCluster,
    volume: &mut W,
    rng: &mut XoroshiroRandom,
    center: BlockPos,
    column_height: i32,
    reach: i32,
) -> bool
where
    W: WorldGenVolume,
{
    let mut placed_any = false;
    for dz in -reach..=reach {
        for dx in -reach..=reach {
            let at = center + IVec3::new(dx, 0, dz);
            let step_limit = dist_manhattan(at.as_ivec3(), center.as_ivec3());
            let start = if config.can_replace.test(volume, at) {
                find_surface(config, volume, at, step_limit)
            } else {
                find_air(config, volume, at, step_limit)
            };
            let Some(mut cursor) = start else {
                continue;
            };
            let mut remaining = column_height - step_limit / 2;
            while remaining >= 0 {
                if config.can_replace.test(volume, cursor) {
                    let state = config.block.state(volume, rng, cursor);
                    volume.set(cursor, state);
                    placed_any = true;
                } else if !config.continue_through.test(volume, cursor) {
                    break;
                }
                cursor += IVec3::Y;
                remaining -= 1;
            }
        }
    }
    placed_any
}

/// A cell at the column's own centre has a step limit of zero, which both
/// searches read as no budget at all, so the centre never seeds a column.
fn find_surface<W: WorldGenVolume>(
    config: &CompiledSteppedColumnCluster,
    volume: &W,
    from: BlockPos,
    mut limit: i32,
) -> Option<BlockPos> {
    let floor = volume.extent().min_y + 1;
    let mut cursor = from;
    while cursor.y > floor && limit > 0 {
        limit -= 1;
        if can_place_at(config, volume, cursor) {
            return Some(cursor);
        }
        cursor -= IVec3::Y;
    }
    None
}

fn find_air<W: WorldGenVolume>(
    config: &CompiledSteppedColumnCluster,
    volume: &W,
    from: BlockPos,
    mut limit: i32,
) -> Option<BlockPos> {
    let extent = volume.extent();
    let ceiling = extent.min_y + extent.depth - 1;
    let mut cursor = from;
    while cursor.y <= ceiling && limit > 0 {
        limit -= 1;
        if volume.holds(&config.cannot_place_on, cursor) {
            return None;
        }
        if volume.is_air(cursor) {
            return Some(cursor);
        }
        cursor += IVec3::Y;
    }
    None
}

fn can_place_at<W: WorldGenVolume>(
    config: &CompiledSteppedColumnCluster,
    volume: &W,
    pos: BlockPos,
) -> bool {
    if !config.can_replace.test(volume, pos) {
        return false;
    }
    let below = pos - IVec3::Y;
    !volume.is_air(below) && !volume.holds(&config.cannot_place_on, below)
}

#[cfg(test)]
mod tests {
    use mcrs_minecraft_worldgen::feature::placer::mask_of;

    use mcrs_minecraft_random::xoroshiro::XoroshiroRandom;
    use mcrs_voxel_storage::VoxelId;

    use super::*;
    use crate::feature::tree::provider::fake::{AIR, FakeVolume};

    const BASALT: VoxelId = VoxelId(4);
    const STONE: VoxelId = VoxelId(5);
    const BLACKSTONE: VoxelId = VoxelId(6);
    const AT: BlockPos = BlockPos::new(0, 50, 0);

    fn config(
        cluster_reach: IntProvider,
        column_count: IntProvider,
        column_reach: IntProvider,
        height: IntProvider,
    ) -> CompiledSteppedColumnCluster {
        CompiledSteppedColumnCluster {
            block: StateProvider::Simple(BASALT),
            continue_through: Predicate::MatchingStates {
                offset: IVec3::ZERO,
                states: mask_of([BASALT]),
            },
            can_replace: Predicate::MatchingStates {
                offset: IVec3::ZERO,
                states: mask_of([AIR]),
            },
            cannot_place_on: mask_of([BLACKSTONE]),
            cluster_reach,
            column_count,
            column_reach,
            height,
        }
    }

    /// A floor of stone under the whole cluster, so any cell can seed a column.
    fn floor() -> FakeVolume {
        let cells = (-8..=8).flat_map(|x| (-8..=8).map(move |z| ((x, AT.y - 1, z), STONE)));
        FakeVolume::with(cells)
    }

    /// Nothing under the origin is a refusal, and it costs no draw.
    #[test]
    fn a_cluster_over_air_refuses_before_it_draws() {
        let config = config(
            IntProvider::Constant(2),
            IntProvider::Constant(3),
            IntProvider::Constant(1),
            IntProvider::Constant(5),
        );
        let mut volume = FakeVolume::default();
        let mut rng = XoroshiroRandom::new(6);
        let before = rng.clone();

        assert!(!place_stepped_column_cluster(
            &config,
            &mut volume,
            &mut rng,
            AT
        ));
        assert_eq!(rng, before);
        assert!(volume.writes.is_empty());
    }

    /// A blacklisted block under the origin is refused the same way.
    #[test]
    fn a_cluster_over_a_forbidden_block_refuses() {
        let config = config(
            IntProvider::Constant(2),
            IntProvider::Constant(3),
            IntProvider::Constant(1),
            IntProvider::Constant(5),
        );
        let mut volume = FakeVolume::with([((AT.x, AT.y - 1, AT.z), BLACKSTONE)]);
        let mut rng = XoroshiroRandom::new(6);

        assert!(!place_stepped_column_cluster(
            &config,
            &mut volume,
            &mut rng,
            AT
        ));
    }

    /// Each column spends three bounded ints for its position — the y span is
    /// one cell wide and still draws — and one more for its reach whenever the
    /// position is inside the cluster's height.
    #[test]
    fn every_column_draws_three_positions_and_a_reach() {
        let config = config(
            IntProvider::Constant(2),
            IntProvider::Constant(4),
            IntProvider::uniform(2, 3),
            IntProvider::Constant(5),
        );
        let mut volume = floor();
        let mut rng = XoroshiroRandom::new(19);

        assert!(place_stepped_column_cluster(
            &config,
            &mut volume,
            &mut rng,
            AT
        ));

        let mut replay = XoroshiroRandom::new(19);
        for _ in 0..4 {
            let x = AT.x - 2 + replay.next_i32_bound(5);
            let y = AT.y + replay.next_i32_bound(1);
            let z = AT.z - 2 + replay.next_i32_bound(5);
            if 5 - dist_manhattan(IVec3::new(x, y, z), AT.as_ivec3()) >= 0 {
                replay.next_i32_bound(2);
            }
        }
        assert_eq!(rng, replay, "a simple provider draws nothing per block");
    }

    /// A reach of zero leaves only the column's own centre, whose step limit is
    /// zero, so the cluster writes nothing at all.
    #[test]
    fn a_column_of_no_reach_writes_nothing() {
        let config = config(
            IntProvider::Constant(2),
            IntProvider::Constant(4),
            IntProvider::Constant(0),
            IntProvider::Constant(5),
        );
        let mut volume = floor();
        let mut rng = XoroshiroRandom::new(19);

        assert!(!place_stepped_column_cluster(
            &config,
            &mut volume,
            &mut rng,
            AT
        ));
        assert!(volume.writes.is_empty());
    }

    /// A cluster reach of zero pins the one column on the origin, so its ring
    /// is exactly known: four cells one step out stack the full height, four
    /// diagonals lose one to the halved step, and the centre stays empty.
    #[test]
    fn a_column_steps_down_as_it_leaves_its_own_centre() {
        let config = config(
            IntProvider::Constant(0),
            IntProvider::Constant(1),
            IntProvider::Constant(1),
            IntProvider::Constant(3),
        );
        let mut volume = floor();
        let mut rng = XoroshiroRandom::new(19);

        assert!(place_stepped_column_cluster(
            &config,
            &mut volume,
            &mut rng,
            AT
        ));

        let mut heights: std::collections::HashMap<(i32, i32), i32> =
            std::collections::HashMap::new();
        for ((x, _, z), state) in &volume.writes {
            assert_eq!(*state, BASALT);
            *heights.entry((*x, *z)).or_default() += 1;
        }
        let mut columns: Vec<((i32, i32), i32)> = heights.into_iter().collect();
        columns.sort();
        assert_eq!(
            columns,
            vec![
                ((-1, -1), 3),
                ((-1, 0), 4),
                ((-1, 1), 3),
                ((0, -1), 4),
                ((0, 1), 4),
                ((1, -1), 3),
                ((1, 0), 4),
                ((1, 1), 3),
            ]
        );
    }
}
