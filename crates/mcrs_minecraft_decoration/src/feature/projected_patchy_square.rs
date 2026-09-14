use bevy_math::IVec3;
use mcrs_minecraft_random::Random;
use mcrs_minecraft_random::xoroshiro::XoroshiroRandom;
use mcrs_minecraft_worldgen::feature::placer::{Predicate, WorldGenVolume};
use mcrs_minecraft_worldgen::value_provider::IntProvider;
use mcrs_voxel_math::BlockPos;

use crate::feature::tree::provider::StateProvider;

/// One `projected_random_patchy_square` feature with every name it carries
/// already resolved.
#[derive(Clone, Debug)]
pub struct CompiledProjectedPatchySquare {
    pub block: StateProvider,
    pub project_through: Predicate,
    pub size: IntProvider,
    pub max_projection_height: i32,
}

/// `ProjectedRandomPatchySquare.place`: a square of cells, each kept by a draw
/// that the corners lose more often, then dropped onto whatever it lands on.
///
/// Every cell of the square spends its draw whether or not it is kept, so a
/// rejected corner still moves everything after it.
pub fn place_projected_random_patchy_square<W>(
    config: &CompiledProjectedPatchySquare,
    volume: &mut W,
    rng: &mut XoroshiroRandom,
    origin: BlockPos,
) -> bool
where
    W: WorldGenVolume,
{
    let size = config.size.sample(rng);
    let bound = size * size + 1;
    for dx in -size..=size {
        for dz in -size..=size {
            if rng.next_i32_bound(bound) >= bound - dx.abs() * dz.abs() {
                continue;
            }
            let mut pos = origin + IVec3::new(dx, 0, dz);
            let mut drop = config.max_projection_height;
            while config.project_through.test(volume, pos - IVec3::Y) {
                pos -= IVec3::Y;
                drop -= 1;
                if drop <= 0 {
                    break;
                }
            }
            if let Some(state) = config.block.optional_state(volume, rng, pos) {
                volume.set(pos, state);
            }
        }
    }
    true
}

#[cfg(test)]
mod tests {

    use mcrs_minecraft_random::xoroshiro::XoroshiroRandom;
    use mcrs_minecraft_worldgen::feature::placer::single_state;
    use mcrs_voxel_storage::VoxelId;

    use super::*;
    use crate::feature::tree::provider::fake::{AIR, FakeVolume};

    const BASALT: VoxelId = VoxelId(6);
    const STONE: VoxelId = VoxelId(7);
    const AT: BlockPos = BlockPos::new(0, 70, 0);

    fn config(size: i32, max_projection_height: i32) -> CompiledProjectedPatchySquare {
        CompiledProjectedPatchySquare {
            block: StateProvider::Simple(BASALT),
            project_through: Predicate::MatchingStates {
                offset: IVec3::ZERO,
                states: single_state(AIR),
            },
            size: IntProvider::Constant(size),
            max_projection_height,
        }
    }

    /// A square of side `2 * size + 1` spends exactly one draw per cell, in
    /// x-major then z order, whatever it keeps.
    #[test]
    fn every_cell_of_the_square_costs_one_draw() {
        let config = config(3, 0);
        let mut volume = FakeVolume::default();
        let mut rng = XoroshiroRandom::new(31);

        assert!(place_projected_random_patchy_square(
            &config,
            &mut volume,
            &mut rng,
            AT
        ));

        let mut replay = XoroshiroRandom::new(31);
        for _ in 0..49 {
            replay.next_i32_bound(10);
        }
        assert_eq!(
            rng, replay,
            "a constant size and a simple provider draw once"
        );
    }

    /// The centre is never rejected — its probability is zero — and a cell it
    /// cannot project through writes where it stands.
    #[test]
    fn a_square_of_one_cell_writes_the_origin() {
        let config = config(0, 3);
        let mut volume = FakeVolume::with([((AT.x, AT.y - 1, AT.z), STONE)]);
        let mut rng = XoroshiroRandom::new(8);

        assert!(place_projected_random_patchy_square(
            &config,
            &mut volume,
            &mut rng,
            AT
        ));
        assert_eq!(volume.writes, vec![((AT.x, AT.y, AT.z), BASALT)]);
    }

    /// A cell over air falls until it runs out of allowance, and lands on the
    /// first block it cannot project through even when allowance remains.
    #[test]
    fn a_cell_falls_through_air_up_to_its_allowance() {
        for (allowance, expected) in [(1, AT.y - 1), (3, AT.y - 3), (9, AT.y - 5)] {
            let config = config(0, allowance);
            let mut volume = FakeVolume::with([((AT.x, AT.y - 6, AT.z), STONE)]);
            let mut rng = XoroshiroRandom::new(8);

            assert!(place_projected_random_patchy_square(
                &config,
                &mut volume,
                &mut rng,
                AT
            ));
            assert_eq!(
                volume.writes,
                vec![((AT.x, expected, AT.z), BASALT)],
                "allowance {allowance}"
            );
        }
    }
}
