use crate::random_direction;
use glam::IVec3;
use mcrs_minecraft_chunk::VoxelId;
use mcrs_minecraft_core::Direction;
use mcrs_minecraft_random::Random;
use mcrs_minecraft_random::worldgen::WorldgenRandom;
use mcrs_minecraft_worldgen_feature::placer::{StateMask, WorldGenVolume};
use mcrs_voxel::BlockPos;

use crate::speleothem::{Growth, PointedStates};

#[derive(Clone, Debug)]
pub struct CompiledSpeleothem {
    pub base_block: VoxelId,
    pub pointed: PointedStates,
    pub replaceable_blocks: StateMask,
    pub base_or_replaceable: StateMask,
    pub chance_of_taller_generation: f32,
    pub chance_of_directional_spread: f32,
    pub chance_of_spread_radius2: f32,
    pub chance_of_spread_radius3: f32,
}

pub fn place_speleothem<W: WorldGenVolume>(
    config: &CompiledSpeleothem,
    volume: &mut W,
    rng: &mut WorldgenRandom,
    origin: BlockPos,
) -> bool {
    let above = volume.holds(&config.base_or_replaceable, origin + IVec3::Y);
    let below = volume.holds(&config.base_or_replaceable, origin - IVec3::Y);
    let tip_up = match (above, below) {
        (true, true) => !rng.next_bool(),
        (true, false) => false,
        (false, true) => true,
        (false, false) => return false,
    };

    let tip = if tip_up { IVec3::Y } else { IVec3::NEG_Y };
    place_patch_of_base_blocks(config, volume, rng, origin - tip);
    let taller = rng.next_f32() < config.chance_of_taller_generation && {
        let above = origin + tip;
        volume.world().is_empty_or_water(volume.get(above))
    };
    Growth {
        pointed: &config.pointed,
        root: &config.base_or_replaceable,
        set_attached: false,
    }
    .grow(volume, origin, tip_up, if taller { 2 } else { 1 }, false);
    true
}

fn place_patch_of_base_blocks<W: WorldGenVolume>(
    config: &CompiledSpeleothem,
    volume: &mut W,
    rng: &mut WorldgenRandom,
    root: BlockPos,
) {
    place_base_block_if_possible(config, volume, root);
    for direction in Direction::HORIZONTAL {
        if rng.next_f32() > config.chance_of_directional_spread {
            continue;
        }
        let first = root + direction.normal();
        place_base_block_if_possible(config, volume, first);
        if rng.next_f32() > config.chance_of_spread_radius2 {
            continue;
        }
        let second = first + random_direction(rng).normal();
        place_base_block_if_possible(config, volume, second);
        if rng.next_f32() > config.chance_of_spread_radius3 {
            continue;
        }
        let third = second + random_direction(rng).normal();
        place_base_block_if_possible(config, volume, third);
    }
}

fn place_base_block_if_possible<W: WorldGenVolume>(
    config: &CompiledSpeleothem,
    volume: &mut W,
    pos: BlockPos,
) {
    if volume.holds(&config.replaceable_blocks, pos) {
        volume.set(pos, config.base_block);
    }
}

#[cfg(test)]
mod tests {
    use mcrs_minecraft_worldgen_feature::placer::mask_of;

    use mcrs_minecraft_random::worldgen::WorldgenRandom;

    use super::*;
    use crate::speleothem::{FRUSTUM, TIP};
    use crate::tree::provider::fake::FakeVolume;

    const STONE: VoxelId = VoxelId(1);
    const WATER: VoxelId = VoxelId(2);
    const BASE: VoxelId = VoxelId(3);
    const POINTED: VoxelId = VoxelId(10);

    const AT: BlockPos = BlockPos::new(0, 64, 0);

    fn config() -> CompiledSpeleothem {
        CompiledSpeleothem {
            base_block: BASE,
            pointed: crate::speleothem::tests::pointed_states(POINTED.0),
            replaceable_blocks: mask_of([STONE]),
            base_or_replaceable: mask_of([STONE, BASE]),
            chance_of_taller_generation: 0.2,
            chance_of_directional_spread: 0.7,
            chance_of_spread_radius2: 0.5,
            chance_of_spread_radius3: 0.5,
        }
    }

    #[test]
    fn a_speleothem_with_no_base_either_side_draws_nothing() {
        let config = config();
        let mut volume = FakeVolume::default();
        let mut rng = WorldgenRandom::new(4);
        let before = rng.clone();

        assert!(!place_speleothem(&config, &mut volume, &mut rng, AT));
        assert_eq!(rng, before);
        assert!(volume.writes.is_empty());
    }

    #[test]
    fn the_tip_direction_costs_a_boolean_only_when_both_sides_can_root_it() {
        let ceiling = FakeVolume::with([((AT.x, AT.y + 1, AT.z), STONE)]);
        let floor = FakeVolume::with([((AT.x, AT.y - 1, AT.z), STONE)]);
        let both = FakeVolume::with([
            ((AT.x, AT.y + 1, AT.z), STONE),
            ((AT.x, AT.y - 1, AT.z), STONE),
        ]);
        let config = config();

        for (mut volume, boolean) in [(ceiling, false), (floor, false), (both, true)] {
            let mut rng = WorldgenRandom::new(11);
            let mut replay = WorldgenRandom::new(11);

            assert!(place_speleothem(&config, &mut volume, &mut rng, AT));

            if boolean {
                replay.next_bool();
            }
            replay_patch(&config, &mut replay);
            replay.next_f32();
            assert_eq!(rng, replay);
        }
    }

    fn replay_patch(config: &CompiledSpeleothem, rng: &mut WorldgenRandom) {
        for _ in 0..Direction::HORIZONTAL.len() {
            if rng.next_f32() > config.chance_of_directional_spread {
                continue;
            }
            if rng.next_f32() > config.chance_of_spread_radius2 {
                continue;
            }
            rng.next_i32_bound(6);
            if rng.next_f32() > config.chance_of_spread_radius3 {
                continue;
            }
            rng.next_i32_bound(6);
        }
    }

    #[test]
    fn a_taller_stalactite_writes_a_frustum_then_a_tip_pointing_down() {
        let mut config = config();
        config.chance_of_taller_generation = 1.0;
        config.chance_of_directional_spread = 0.0;
        let mut volume = FakeVolume::with([((AT.x, AT.y + 1, AT.z), STONE)]);
        let mut rng = WorldgenRandom::new(7);

        assert!(place_speleothem(&config, &mut volume, &mut rng, AT));

        let down = |thickness: usize| VoxelId(POINTED.0 + 10 + thickness as u16 * 2);
        assert_eq!(
            volume.writes,
            vec![
                ((AT.x, AT.y + 1, AT.z), BASE),
                ((AT.x, AT.y, AT.z), down(FRUSTUM)),
                ((AT.x, AT.y - 1, AT.z), down(TIP)),
            ]
        );
    }

    #[test]
    fn a_short_stalagmite_in_water_writes_one_waterlogged_tip_pointing_up() {
        let mut config = config();
        config.chance_of_taller_generation = 0.0;
        config.chance_of_directional_spread = 0.0;
        let mut volume =
            FakeVolume::with([((AT.x, AT.y - 1, AT.z), STONE), ((AT.x, AT.y, AT.z), WATER)]);
        volume.world.water_states = mask_of([WATER]);
        volume.world.water_fluid = mask_of([WATER]);
        let mut rng = WorldgenRandom::new(7);

        assert!(place_speleothem(&config, &mut volume, &mut rng, AT));

        assert_eq!(
            volume.writes,
            vec![
                ((AT.x, AT.y - 1, AT.z), BASE),
                ((AT.x, AT.y, AT.z), VoxelId(POINTED.0 + TIP as u16 * 2 + 1)),
            ]
        );
    }

    #[test]
    fn the_patch_only_replaces_replaceable_blocks() {
        let mut config = config();
        config.chance_of_directional_spread = 1.0;
        config.chance_of_spread_radius2 = 0.0;
        config.chance_of_taller_generation = 0.0;
        let stone = [
            (AT.x, AT.y + 1, AT.z),
            (AT.x + 1, AT.y + 1, AT.z),
            (AT.x, AT.y + 1, AT.z - 1),
        ];
        let mut volume = FakeVolume::with(stone.map(|at| (at, STONE)));
        let mut rng = WorldgenRandom::new(21);

        assert!(place_speleothem(&config, &mut volume, &mut rng, AT));

        let base: Vec<_> = volume
            .writes
            .iter()
            .filter(|(_, state)| *state == BASE)
            .map(|(at, _)| *at)
            .collect();
        assert_eq!(base, vec![stone[0], stone[2], stone[1]]);
    }
}
