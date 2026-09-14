use mcrs_minecraft_random::Random;
use mcrs_minecraft_random::xoroshiro::XoroshiroRandom;
use mcrs_minecraft_worldgen::feature::block_predicate::Direction;
use mcrs_minecraft_worldgen::feature::placer::{Predicate, WorldGenVolume};
use mcrs_voxel_math::BlockPos;

use crate::feature::tree::provider::StateProvider;

/// One `single_block_pillar` feature with every name it carries already
/// resolved.
#[derive(Clone, Debug)]
pub struct CompiledSingleBlockPillar {
    pub block: StateProvider,
    pub can_replace: Predicate,
    pub direction: Direction,
    pub chance_to_continue: f32,
}

/// `SingleBlockPillarFeature.place`: one cell at a time until the column is
/// blocked, the chance draw fails or it leaves the world, then the cap feature
/// on the last cell the column actually took.
///
/// The chance is drawn before the build-height test and after the replaceable
/// test, so a column stopped by either still costs what it drew.
pub fn place_single_block_pillar<W>(
    config: &CompiledSingleBlockPillar,
    volume: &mut W,
    rng: &mut XoroshiroRandom,
    origin: BlockPos,
    place_cap: &mut dyn FnMut(&mut W, &mut XoroshiroRandom, BlockPos) -> bool,
) -> bool
where
    W: WorldGenVolume,
{
    let mut pos = origin;
    while config.can_replace.test(volume, pos)
        && rng.next_f32() < config.chance_to_continue
        && volume.extent().contains(pos.y)
    {
        let state = config.block.state(volume, rng, pos);
        volume.set(pos, state);
        pos += config.direction.normal();
    }
    place_cap(volume, rng, pos + config.direction.opposite().normal());
    true
}

#[cfg(test)]
mod tests {

    use mcrs_minecraft_random::xoroshiro::XoroshiroRandom;
    use mcrs_minecraft_worldgen::feature::placer::single_state;
    use mcrs_voxel_storage::VoxelId;

    use bevy_math::IVec3;

    use super::*;
    use crate::feature::tree::provider::fake::{AIR, FakeVolume};

    const BASALT: VoxelId = VoxelId(7);
    const STONE: VoxelId = VoxelId(8);
    const AT: BlockPos = BlockPos::new(2, 40, -5);

    fn config(chance_to_continue: f32) -> CompiledSingleBlockPillar {
        CompiledSingleBlockPillar {
            block: StateProvider::Simple(BASALT),
            can_replace: Predicate::MatchingStates {
                offset: IVec3::ZERO,
                states: single_state(AIR),
            },
            direction: Direction::Down,
            chance_to_continue,
        }
    }

    /// A column that always continues runs down to the build-height floor, one
    /// float per cell, and hands the cap the last cell it wrote.
    #[test]
    fn a_certain_column_runs_to_the_floor_and_caps_its_last_cell() {
        let config = config(1.0);
        let mut volume = FakeVolume::default();
        let mut rng = XoroshiroRandom::new(13);
        let mut capped = Vec::new();

        assert!(place_single_block_pillar(
            &config,
            &mut volume,
            &mut rng,
            AT,
            &mut |_, _, pos| {
                capped.push(pos);
                true
            }
        ));

        let floor = volume.extent().min_y;
        let written: Vec<i32> = volume.writes.iter().map(|((_, y, _), _)| *y).collect();
        assert_eq!(written, (floor..=AT.y).rev().collect::<Vec<_>>());
        assert_eq!(capped, vec![BlockPos::from(AT.with_y(floor))]);

        let mut replay = XoroshiroRandom::new(13);
        for _ in 0..=(AT.y - floor) {
            replay.next_f32();
        }
        replay.next_f32();
        assert_eq!(
            rng, replay,
            "one float per cell plus the one that stopped it"
        );
    }

    /// A blocked origin writes nothing and draws nothing, and the cap still
    /// runs — one cell back the way the column would have gone.
    #[test]
    fn a_blocked_origin_still_caps_one_cell_behind_it() {
        let config = config(1.0);
        let mut volume = FakeVolume::with([((AT.x, AT.y, AT.z), STONE)]);
        let mut rng = XoroshiroRandom::new(1);
        let before = rng.clone();
        let mut capped = Vec::new();

        assert!(place_single_block_pillar(
            &config,
            &mut volume,
            &mut rng,
            AT,
            &mut |_, _, pos| {
                capped.push(pos);
                true
            }
        ));

        assert_eq!(rng, before);
        assert!(volume.writes.is_empty());
        assert_eq!(capped, vec![AT + IVec3::Y]);
    }

    /// A chance of zero stops the column on its first draw, so the origin is
    /// never written and the cap lands above it.
    #[test]
    fn a_chance_of_zero_spends_one_draw_and_writes_nothing() {
        let config = config(0.0);
        let mut volume = FakeVolume::default();
        let mut rng = XoroshiroRandom::new(2);
        let mut capped = Vec::new();

        assert!(place_single_block_pillar(
            &config,
            &mut volume,
            &mut rng,
            AT,
            &mut |_, _, pos| {
                capped.push(pos);
                true
            }
        ));

        let mut replay = XoroshiroRandom::new(2);
        replay.next_f32();
        assert_eq!(rng, replay);
        assert!(volume.writes.is_empty());
        assert_eq!(capped, vec![AT + IVec3::Y]);
    }
}
