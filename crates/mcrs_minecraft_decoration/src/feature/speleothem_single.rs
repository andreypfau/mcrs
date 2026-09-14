use crate::feature::random_direction;
use bevy_math::IVec3;
use mcrs_minecraft_core::BlockPos;
use mcrs_minecraft_random::Random;
use mcrs_minecraft_random::xoroshiro::XoroshiroRandom;
use mcrs_minecraft_worldgen::feature::block_predicate::Direction;
use mcrs_minecraft_worldgen::feature::placer::{StateMask, WorldGenVolume};
use mcrs_voxel_storage::VoxelId;

use crate::feature::speleothem::{PointedStates, grow_speleothem};

/// One `speleothem` feature with every name it carries already resolved.
#[derive(Clone, Debug)]
pub struct CompiledSpeleothem {
    pub base_block: VoxelId,
    pub pointed: PointedStates,
    /// `placeBaseBlockIfPossible`, which asks for the replaceable set alone.
    pub replaceable_blocks: StateMask,
    /// `SpeleothemUtils.isBase`: the base block or one of the replaceables.
    pub base_or_replaceable: StateMask,
    pub chance_of_taller_generation: f32,
    pub chance_of_directional_spread: f32,
    pub chance_of_spread_radius2: f32,
    pub chance_of_spread_radius3: f32,
}

/// `SpeleothemFeature.place`: pick which way the tip points, spread a patch of
/// base blocks behind it, then grow one or two pointed blocks.
pub fn place_speleothem<W: WorldGenVolume>(
    config: &CompiledSpeleothem,
    volume: &mut W,
    rng: &mut XoroshiroRandom,
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
    grow_speleothem(
        &config.pointed,
        &config.base_or_replaceable,
        volume,
        origin,
        tip_up,
        if taller { 2 } else { 1 },
        false,
    );
    true
}

fn place_patch_of_base_blocks<W: WorldGenVolume>(
    config: &CompiledSpeleothem,
    volume: &mut W,
    rng: &mut XoroshiroRandom,
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
    use mcrs_minecraft_worldgen::feature::placer::mask_of;

    use mcrs_minecraft_random::xoroshiro::XoroshiroRandom;

    use super::*;
    use crate::feature::speleothem::{FRUSTUM, TIP};
    use crate::feature::tree::provider::fake::FakeVolume;

    const STONE: VoxelId = VoxelId(1);
    const WATER: VoxelId = VoxelId(2);
    const BASE: VoxelId = VoxelId(3);
    /// `[up, down][tip_merge, tip, frustum, middle, base][dry, wet]`, laid out
    /// so a written state reads back as its three indices.
    const POINTED: VoxelId = VoxelId(10);

    const AT: BlockPos = BlockPos::new(0, 64, 0);

    fn config() -> CompiledSpeleothem {
        CompiledSpeleothem {
            base_block: BASE,
            pointed: crate::feature::speleothem::tests::pointed_states(POINTED.0),
            replaceable_blocks: mask_of([STONE]),
            base_or_replaceable: mask_of([STONE, BASE]),
            chance_of_taller_generation: 0.2,
            chance_of_directional_spread: 0.7,
            chance_of_spread_radius2: 0.5,
            chance_of_spread_radius3: 0.5,
        }
    }

    /// Neither side can root the column, so the feature refuses before it draws
    /// anything at all.
    #[test]
    fn a_speleothem_with_no_base_either_side_draws_nothing() {
        let config = config();
        let mut volume = FakeVolume::default();
        let mut rng = XoroshiroRandom::new(4);
        let before = rng.clone();

        assert!(!place_speleothem(&config, &mut volume, &mut rng, AT));
        assert_eq!(rng, before);
        assert!(volume.writes.is_empty());
    }

    /// Stone above and below spends the direction boolean; stone on one side
    /// only does not, and the tip points away from the block it roots in.
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
            let mut rng = XoroshiroRandom::new(11);
            let mut replay = XoroshiroRandom::new(11);

            assert!(place_speleothem(&config, &mut volume, &mut rng, AT));

            if boolean {
                replay.next_bool();
            }
            replay_patch(&config, &mut replay);
            replay.next_f32();
            assert_eq!(rng, replay);
        }
    }

    /// `createPatchOfBaseBlocks` walks the four horizontals in plane order and
    /// spends a float per ring, plus a whole-direction draw per ring it takes.
    fn replay_patch(config: &CompiledSpeleothem, rng: &mut XoroshiroRandom) {
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

    /// A stalactite roots in the ceiling and writes downward: the frustum where
    /// the object landed and the tip below it, both pointing down.
    #[test]
    fn a_taller_stalactite_writes_a_frustum_then_a_tip_pointing_down() {
        let mut config = config();
        config.chance_of_taller_generation = 1.0;
        config.chance_of_directional_spread = 0.0;
        let mut volume = FakeVolume::with([((AT.x, AT.y + 1, AT.z), STONE)]);
        let mut rng = XoroshiroRandom::new(7);

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

    /// A stalagmite roots in the floor and writes upward; water at the cell
    /// takes the waterlogged state of the same thickness.
    #[test]
    fn a_short_stalagmite_in_water_writes_one_waterlogged_tip_pointing_up() {
        let mut config = config();
        config.chance_of_taller_generation = 0.0;
        config.chance_of_directional_spread = 0.0;
        let mut volume =
            FakeVolume::with([((AT.x, AT.y - 1, AT.z), STONE), ((AT.x, AT.y, AT.z), WATER)]);
        volume.world.water_states = mask_of([WATER]);
        volume.world.water_fluid = mask_of([WATER]);
        let mut rng = XoroshiroRandom::new(7);

        assert!(place_speleothem(&config, &mut volume, &mut rng, AT));

        assert_eq!(
            volume.writes,
            vec![
                ((AT.x, AT.y - 1, AT.z), BASE),
                ((AT.x, AT.y, AT.z), VoxelId(POINTED.0 + TIP as u16 * 2 + 1)),
            ]
        );
    }

    /// The patch replaces only what is in the replaceable set, and a ring that
    /// lands on the base block it just wrote leaves it alone.
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
        let mut rng = XoroshiroRandom::new(21);

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
