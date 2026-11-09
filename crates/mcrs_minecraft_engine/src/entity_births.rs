use bevy_ecs::prelude::With;
use bevy_ecs::system::{Query, Res, SystemParam};
use mcrs_minecraft_dimension::Dimension as DimensionKey;
use mcrs_minecraft_engine_world::world::dimension::Dimension;
use mcrs_minecraft_entity_key::{BirthSource, EntityKey};
use mcrs_minecraft_resource::ResourceKeyBuf;

use crate::coordinator::birth_clock::BirthClock;
use crate::world_seed::WorldSeed;

/// The inputs of the new UUIDs of this world: the world seed, the key of its dimension and the
/// birth clock. None of them depends on the cells of the dimension. A world without a dimension
/// entity hashes an empty dimension key.
#[derive(SystemParam)]
pub struct EntityBirths<'w, 's> {
    clock: Res<'w, BirthClock>,
    seed: Option<Res<'w, WorldSeed>>,
    dimensions: Query<'w, 's, &'static ResourceKeyBuf<DimensionKey>, With<Dimension>>,
}

impl EntityBirths<'_, '_> {
    #[must_use]
    pub fn tick(&self) -> u64 {
        self.clock.0
    }

    /// The UUID of the entity number `sequence` that `source` brings into existence in this
    /// tick (PHY-CMP-013).
    #[must_use]
    pub fn born(&self, source: &BirthSource, sequence: u32) -> EntityKey {
        let dimension = self.dimensions.single().map_or("", |key| key.as_str());
        let seed = self.seed.as_deref().map_or(0, |seed| seed.0);
        EntityKey::born(seed, dimension, self.clock.0, source, sequence)
    }
}

#[cfg(test)]
mod tests {
    use std::num::NonZeroU32;

    use bevy_ecs::system::RunSystemOnce;
    use bevy_ecs::world::World;
    use mcrs_engine_cell::CellIndex;
    use mcrs_minecraft_engine_world::owned_columns::{OwnedColumns, region_partition};
    use mcrs_minecraft_entity_key::BirthSequences;
    use mcrs_minecraft_resource::ResourceLocationBuf;
    use mcrs_voxel::{BlockPos, ColumnPos};

    use super::*;

    /// Two summons of tick 9 in the Overworld: each at its own spawn point, from a source in the
    /// column of its own cell when the dimension has two cells.
    const SUMMONS: [BirthSource; 2] = [
        BirthSource::Command {
            spawn: BlockPos::new(4, 64, 4),
            source: BlockPos::new(1, 64, 1),
        },
        BirthSource::Command {
            spawn: BlockPos::new(8, 64, 8),
            source: BlockPos::new(600, 64, 1),
        },
    ];

    /// A world of the Overworld at tick 9; `cell` of two cells, or the only cell.
    fn world(cell: Option<u32>) -> World {
        let mut world = World::new();
        world.insert_resource(BirthClock(9));
        world.insert_resource(WorldSeed(5));
        if let (Some(cell), Some(cells)) = (cell, NonZeroU32::new(2)) {
            world.insert_resource(OwnedColumns::new(region_partition(cells), CellIndex(cell)));
        }
        let location = ResourceLocationBuf::read("minecraft:overworld").expect("a valid key");
        world.spawn((
            Dimension,
            ResourceKeyBuf::<DimensionKey>::from_location(location),
        ));
        world
    }

    /// The UUIDs that `world` gives to `summons`, in their order.
    fn summon(world: &mut World, summons: &[BirthSource]) -> Vec<EntityKey> {
        let summons = summons.to_vec();
        world
            .run_system_once(move |births: EntityBirths| {
                let mut sequences = BirthSequences::default();
                summons
                    .iter()
                    .map(|source| births.born(source, sequences.next(births.tick(), source)))
                    .collect::<Vec<EntityKey>>()
            })
            .expect("the system runs")
    }

    #[test]
    fn cmp_013_024_two_summons_of_one_tick_differ_and_keep_their_uuids_across_cell_layouts() {
        let one_cell = summon(&mut world(None), &SUMMONS);
        assert_ne!(one_cell[0], one_cell[1], "two spawn points give two UUIDs");
        let two_cells: Vec<EntityKey> = [0, 1]
            .into_iter()
            .zip(&SUMMONS)
            .flat_map(|(cell, source)| {
                let mut world = world(Some(cell));
                if let BirthSource::Command { source: at, .. } = source {
                    let owned = world.resource::<OwnedColumns>();
                    assert!(
                        owned.owns(ColumnPos::from(*at)),
                        "cell {cell} runs the command"
                    );
                }
                summon(&mut world, &[*source])
            })
            .collect();
        assert_ne!(two_cells[0], two_cells[1], "two cells give two UUIDs");
        assert_eq!(
            one_cell, two_cells,
            "the cell layout does not change a UUID"
        );
    }
}
