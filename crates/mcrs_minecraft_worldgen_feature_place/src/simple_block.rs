use crate::holds;

use bevy_math::IVec3;
use mcrs_minecraft_chunk::VoxelId;
use mcrs_minecraft_core::BlockPos;
use mcrs_minecraft_random::worldgen::WorldgenRandom;
use mcrs_minecraft_worldgen_feature::placer::WorldGenVolume;

use std::sync::Arc;

use crate::mossy_carpet::place_mossy_carpet;
use crate::tables::BlockTables;
use crate::tree::provider::StateProvider;

/// The two halves of a `DoublePlantBlock`, and the same pair waterlogged for a
/// block that carries the property.
#[derive(Clone, Debug)]
pub struct DoublePlant {
    pub lower: VoxelId,
    pub upper: VoxelId,
    pub waterlogged: Option<(VoxelId, VoxelId)>,
}

impl DoublePlant {
    fn half(&self, upper: bool, waterlogged: bool) -> VoxelId {
        match (waterlogged, self.waterlogged) {
            (true, Some((wet_lower, wet_upper))) => {
                if upper {
                    wet_upper
                } else {
                    wet_lower
                }
            }
            _ if upper => self.upper,
            _ => self.lower,
        }
    }
}

/// One `simple_block` feature with every name it carries already resolved.
#[derive(Clone, Debug)]
pub struct CompiledSimpleBlock {
    pub to_place: StateProvider,
    pub tables: Arc<BlockTables>,
}

/// `SimpleBlockFeature.place`: one state from the provider, refused when it
/// could not survive, and a second block for the upper half of a double plant.
pub fn place_simple_block<W: WorldGenVolume>(
    cfg: &CompiledSimpleBlock,
    volume: &mut W,
    rng: &mut WorldgenRandom,
    at: BlockPos,
) -> bool {
    let Some(state) = cfg.to_place.optional_state(volume, rng, at) else {
        return false;
    };
    if !volume.would_survive(state, at) {
        return false;
    }

    let Some(halves) = cfg.tables.double_plants.get(&state.0) else {
        match cfg
            .tables
            .mossy_carpet
            .as_ref()
            .filter(|carpet| carpet.holds(state))
        {
            Some(carpet) => place_mossy_carpet(carpet, volume, at),
            None => volume.set(at, state),
        }
        return true;
    };

    let above = at + IVec3::Y;
    let above_state = volume.get(above);
    let world = volume.world();
    if !holds(&world.air_states, above_state)
        && (holds(&world.water_fluid, state) != holds(&world.water_fluid, above_state)
            || !holds(&world.replaceable, above_state))
    {
        return false;
    }

    let lower_wet = holds(&world.water_fluid, volume.get(at));
    volume.set(at, halves.half(false, lower_wet));
    volume.set(
        above,
        halves.half(true, holds(&volume.world().water_fluid, above_state)),
    );
    true
}

#[cfg(test)]
mod tests {
    use mcrs_minecraft_random::Random;
    use mcrs_minecraft_worldgen_feature::placer::mask_of;

    use mcrs_minecraft_random::worldgen::WorldgenRandom;

    use super::*;
    use crate::tree::provider::fake::{AIR, FakeVolume};

    const FLOWER: VoxelId = VoxelId(5);
    const TALL_LOWER: VoxelId = VoxelId(6);
    const TALL_UPPER: VoxelId = VoxelId(7);
    const TALL_WET_LOWER: VoxelId = VoxelId(8);
    const TALL_WET_UPPER: VoxelId = VoxelId(9);
    const WATER: VoxelId = VoxelId(10);
    const STONE: VoxelId = VoxelId(11);
    const TALL: VoxelId = TALL_LOWER;

    const AT: BlockPos = BlockPos::new(3, 70, -4);

    /// Air and water give way; water and the wet halves hold water.
    fn pond(mut volume: FakeVolume) -> FakeVolume {
        volume.world.replaceable = mask_of([AIR, WATER]);
        volume.world.water_fluid = mask_of([WATER, TALL_WET_LOWER, TALL_WET_UPPER]);
        volume
    }

    fn config(to_place: StateProvider) -> CompiledSimpleBlock {
        CompiledSimpleBlock {
            to_place,
            tables: Arc::new(BlockTables {
                double_plants: [TALL, TALL_WET_LOWER]
                    .into_iter()
                    .map(|state| {
                        (
                            state.0,
                            DoublePlant {
                                lower: TALL_LOWER,
                                upper: TALL_UPPER,
                                waterlogged: Some((TALL_WET_LOWER, TALL_WET_UPPER)),
                            },
                        )
                    })
                    .collect(),
                ..BlockTables::default()
            }),
        }
    }

    /// A plain state is one write and whatever the provider drew, nothing else.
    #[test]
    fn a_plain_state_is_one_write_and_the_providers_own_draws() {
        let cfg = config(StateProvider::Weighted(vec![(FLOWER, 1)]));
        let mut volume = pond(FakeVolume::default());
        let mut rng = WorldgenRandom::new(77);

        assert!(place_simple_block(&cfg, &mut volume, &mut rng, AT));

        let mut replay = WorldgenRandom::new(77);
        replay.next_i32_bound(1);
        assert_eq!(rng, replay, "only the provider draws");
        assert_eq!(volume.writes, vec![((AT.x, AT.y, AT.z), FLOWER)]);
    }

    /// `DoublePlantBlock.placeAt`: the lower half where the object landed and
    /// the upper half above it, dry ground and no draws of its own.
    #[test]
    fn a_double_plant_takes_the_cell_above_it_too() {
        let cfg = config(StateProvider::Simple(TALL));
        let mut volume = pond(FakeVolume::default());
        let mut rng = WorldgenRandom::new(1);
        let before = rng.clone();

        assert!(place_simple_block(&cfg, &mut volume, &mut rng, AT));

        assert_eq!(rng, before, "a simple provider draws nothing");
        assert_eq!(
            volume.writes,
            vec![
                ((AT.x, AT.y, AT.z), TALL_LOWER),
                ((AT.x, AT.y + 1, AT.z), TALL_UPPER),
            ]
        );
    }

    /// `copyWaterloggedFrom`: a plant whose own fluid is water matches the
    /// water above it and takes the waterlogged form of both halves.
    #[test]
    fn a_double_plant_in_water_waterlogs_both_halves() {
        let cfg = config(StateProvider::Simple(TALL_WET_LOWER));
        let mut volume = pond(FakeVolume::with([
            ((AT.x, AT.y, AT.z), WATER),
            ((AT.x, AT.y + 1, AT.z), WATER),
        ]));
        let mut rng = WorldgenRandom::new(1);

        assert!(place_simple_block(&cfg, &mut volume, &mut rng, AT));
        assert_eq!(
            volume.writes,
            vec![
                ((AT.x, AT.y, AT.z), TALL_WET_LOWER),
                ((AT.x, AT.y + 1, AT.z), TALL_WET_UPPER),
            ]
        );
    }

    /// The block above must be air, or share the plant's fluid and be
    /// replaceable; stone above refuses the whole placement.
    #[test]
    fn a_double_plant_is_refused_under_a_solid_block() {
        let cfg = config(StateProvider::Simple(TALL));
        let mut volume = pond(FakeVolume::with([((AT.x, AT.y + 1, AT.z), STONE)]));
        let mut rng = WorldgenRandom::new(1);

        assert!(!place_simple_block(&cfg, &mut volume, &mut rng, AT));
        assert!(volume.writes.is_empty());
    }

    /// Water above a dry plant is a fluid mismatch, which is a refusal even
    /// though water is replaceable.
    #[test]
    fn a_dry_double_plant_is_refused_under_water() {
        let cfg = config(StateProvider::Simple(TALL));
        let mut volume = pond(FakeVolume::with([((AT.x, AT.y + 1, AT.z), WATER)]));
        let mut rng = WorldgenRandom::new(1);

        assert!(!place_simple_block(&cfg, &mut volume, &mut rng, AT));
        assert!(volume.writes.is_empty());
    }

    /// A provider that hands back nothing places nothing, and the draw it spent
    /// looking still counts.
    #[test]
    fn an_empty_provider_places_nothing_and_keeps_its_draw() {
        let cfg = config(StateProvider::Weighted(Vec::new()));
        let mut volume = pond(FakeVolume::default());
        let mut rng = WorldgenRandom::new(5);
        let before = rng.clone();

        assert!(!place_simple_block(&cfg, &mut volume, &mut rng, AT));
        assert_eq!(rng, before, "an empty weighted list draws nothing");
        assert!(volume.writes.is_empty());
    }
}
