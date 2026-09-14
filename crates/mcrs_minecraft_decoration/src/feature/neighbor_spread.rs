use bevy_math::IVec3;
use mcrs_minecraft_random::xoroshiro::XoroshiroRandom;
use mcrs_minecraft_worldgen::feature::block_predicate::Direction;
use mcrs_minecraft_worldgen::feature::placer::{Predicate, StateMask, WorldGenVolume};
use mcrs_minecraft_worldgen::value_provider::IntProvider;
use mcrs_voxel_math::BlockPos;

use crate::feature::tree::provider::StateProvider;

/// One `random_neighbor_spread` feature with every name it carries already
/// resolved.
#[derive(Clone, Debug)]
pub struct CompiledNeighborSpread {
    pub block: StateProvider,
    pub accepted_neighbors: StateMask,
    pub can_replace: Predicate,
    pub attempts: IntProvider,
    pub xz_offset: IntProvider,
    pub y_offset: IntProvider,
}

/// `RandomNeighborSpreadFeature.place`: a block at the origin, then one
/// candidate per attempt that goes in only where exactly one of its six
/// neighbours is already part of the growth.
///
/// The three offsets are drawn for every attempt, whether or not the candidate
/// is anywhere the feature could write.
pub fn place_random_neighbor_spread<W: WorldGenVolume>(
    cfg: &CompiledNeighborSpread,
    volume: &mut W,
    rng: &mut XoroshiroRandom,
    at: BlockPos,
) -> bool {
    let state = cfg.block.state(volume, rng, at);
    volume.set(at, state);

    let attempts = cfg.attempts.sample(rng);
    for _ in 0..attempts {
        let x = cfg.xz_offset.sample(rng);
        let y = cfg.y_offset.sample(rng);
        let z = cfg.xz_offset.sample(rng);
        let pos = at + IVec3::new(x, y, z);
        if !cfg.can_replace.test(volume, pos) {
            continue;
        }

        let mut neighbours = 0;
        for direction in Direction::all() {
            let neighbour = pos + direction.normal();
            if volume.holds(&cfg.accepted_neighbors, neighbour) {
                neighbours += 1;
            }
            if neighbours > 1 {
                break;
            }
        }

        if neighbours == 1 {
            let state = cfg.block.state(volume, rng, pos);
            volume.set(pos, state);
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use mcrs_minecraft_random::Random;
    use mcrs_minecraft_worldgen::feature::placer::mask_of;

    use mcrs_minecraft_random::xoroshiro::XoroshiroRandom;
    use mcrs_minecraft_worldgen::value_provider::DispatchedIntProvider;
    use mcrs_voxel_storage::VoxelId;

    use super::*;
    use crate::feature::tree::provider::fake::{AIR, FakeVolume};

    const GLOWSTONE: VoxelId = VoxelId(1);
    const AT: BlockPos = BlockPos::new(0, 80, 0);

    fn config(attempts: i32) -> CompiledNeighborSpread {
        CompiledNeighborSpread {
            block: StateProvider::Simple(GLOWSTONE),
            accepted_neighbors: mask_of([GLOWSTONE]),
            can_replace: Predicate::MatchingStates {
                offset: IVec3::ZERO,
                states: mask_of([AIR]),
            },
            attempts: IntProvider::Constant(attempts),
            xz_offset: IntProvider::Dispatched(DispatchedIntProvider::Uniform {
                min_inclusive: -2,
                max_inclusive: 2,
            }),
            y_offset: IntProvider::Dispatched(DispatchedIntProvider::Uniform {
                min_inclusive: -2,
                max_inclusive: 0,
            }),
        }
    }

    /// Three offsets per attempt in x, y, z order, spent whether or not the
    /// candidate can be written.
    #[test]
    fn every_attempt_draws_its_three_offsets_in_order() {
        let cfg = config(12);
        let mut volume = FakeVolume::default();
        let mut rng = XoroshiroRandom::new(2024);

        assert!(place_random_neighbor_spread(
            &cfg,
            &mut volume,
            &mut rng,
            AT
        ));

        let mut replay = XoroshiroRandom::new(2024);
        for _ in 0..12 {
            replay.next_int_between_inclusive(-2, 2);
            replay.next_int_between_inclusive(-2, 0);
            replay.next_int_between_inclusive(-2, 2);
        }
        assert_eq!(rng, replay, "three offsets an attempt, and nothing else");

        assert_eq!(volume.writes[0], ((AT.x, AT.y, AT.z), GLOWSTONE));
        assert!(
            volume.writes.len() > 1,
            "the seed block should have grown neighbours: {:?}",
            volume.writes
        );
    }

    /// A candidate touching two of the growth is refused; one is the whole
    /// rule.
    #[test]
    fn a_candidate_with_two_neighbours_is_refused() {
        let mut cfg = config(1);
        cfg.xz_offset = IntProvider::Constant(0);
        cfg.y_offset = IntProvider::Constant(1);
        let mut volume = FakeVolume::with([((AT.x, AT.y + 2, AT.z), GLOWSTONE)]);
        let mut rng = XoroshiroRandom::new(7);

        place_random_neighbor_spread(&cfg, &mut volume, &mut rng, AT);

        assert_eq!(
            volume.writes,
            vec![((AT.x, AT.y, AT.z), GLOWSTONE)],
            "the candidate above the origin sits between two glowstones"
        );
    }
}
