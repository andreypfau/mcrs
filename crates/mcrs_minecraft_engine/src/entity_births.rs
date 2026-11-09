use bevy_ecs::prelude::With;
use bevy_ecs::system::{Query, Res, SystemParam};
use mcrs_minecraft_dimension::Dimension as DimensionKey;
use mcrs_minecraft_engine_world::owned_columns::OwnedColumns;
use mcrs_minecraft_engine_world::world::dimension::Dimension;
use mcrs_minecraft_entity_key::{BirthSource, EntityKey};
use mcrs_minecraft_resource::ResourceKeyBuf;

use crate::coordinator::birth_clock::BirthClock;
use crate::world_seed::WorldSeed;

/// The inputs of the new UUIDs of this world: the world seed, the key of its dimension, its cell
/// and the birth clock. A world without a dimension entity hashes an empty dimension key.
#[derive(SystemParam)]
pub struct EntityBirths<'w, 's> {
    clock: Res<'w, BirthClock>,
    seed: Option<Res<'w, WorldSeed>>,
    owned: Option<Res<'w, OwnedColumns>>,
    dimensions: Query<'w, 's, &'static ResourceKeyBuf<DimensionKey>, With<Dimension>>,
}

impl EntityBirths<'_, '_> {
    #[must_use]
    pub fn tick(&self) -> u64 {
        self.clock.0
    }

    /// The index of this cell in its dimension; 0 in a world of one cell.
    #[must_use]
    pub fn cell(&self) -> u32 {
        self.owned.as_deref().map_or(0, |owned| owned.index().0)
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
    use bevy_ecs::system::RunSystemOnce;
    use bevy_ecs::world::World;
    use mcrs_engine_cell::CellIndex;
    use mcrs_minecraft_engine_world::owned_columns::region_partition;
    use mcrs_minecraft_resource::ResourceLocationBuf;

    use super::*;

    /// The UUID of the first summon of tick 9 in a world of `dimension` and `cell` of `cells`.
    fn first_summon(dimension: &str, cell: u32, cells: u32) -> EntityKey {
        let mut world = World::new();
        world.insert_resource(BirthClock(9));
        world.insert_resource(WorldSeed(5));
        if let Some(count) = std::num::NonZeroU32::new(cells).filter(|count| count.get() > 1) {
            world.insert_resource(OwnedColumns::new(region_partition(count), CellIndex(cell)));
        }
        let location = ResourceLocationBuf::read(dimension).expect("a valid dimension key");
        world.spawn((
            Dimension,
            ResourceKeyBuf::<DimensionKey>::from_location(location),
        ));
        world
            .run_system_once(|births: EntityBirths| {
                births.born(
                    &BirthSource::Command {
                        cell: births.cell(),
                        order: 0,
                    },
                    0,
                )
            })
            .expect("the system runs")
    }

    #[test]
    fn cmp_024_the_first_summons_of_one_tick_differ_across_dimensions_and_cells() {
        let summons = [
            first_summon("minecraft:overworld", 0, 2),
            first_summon("minecraft:overworld", 1, 2),
            first_summon("minecraft:the_nether", 0, 2),
            first_summon("minecraft:the_nether", 0, 1),
        ];
        for (index, uuid) in summons.iter().enumerate().take(3) {
            for other in &summons[index + 1..3] {
                assert_ne!(uuid, other, "summon {index} has its own UUID");
            }
        }
        assert_eq!(
            summons[2], summons[3],
            "the first cell of a dimension makes the UUIDs of a world of one cell"
        );
    }
}
