use bevy_derive::Deref;
use bevy_ecs::prelude::{Commands, Component, Entity, Query, Without};
use mcrs_voxel_math::{BlockPos, ChunkPos};

use crate::world::dimension::InDimension;
use crate::world::storage::chunk::ChunkIndex;

/// Back-link from a block entity to the section holding it; the section's
/// despawn takes its block entities with it.
#[derive(Component, Clone, Copy, Debug)]
#[relationship(relationship_target = SectionBlockEntities)]
pub struct InSection(pub Entity);

#[derive(Component, Debug, Default, Deref)]
#[relationship_target(relationship = InSection, linked_spawn)]
pub struct SectionBlockEntities(Vec<Entity>);

/// The block a block entity belongs to, and what resolves its section.
#[derive(Component, Clone, Copy, Debug, Deref)]
pub struct BlockEntityPos(pub BlockPos);

/// A block entity whose section is not loaded has nowhere to live and leaves
/// again: the section is what owns it, and a section that arrives later brings
/// its own from the save or the generator.
pub fn reconcile_block_entities(
    unlinked: Query<(Entity, &BlockEntityPos, &InDimension), Without<InSection>>,
    dimensions: Query<&ChunkIndex>,
    mut commands: Commands,
) {
    for (entity, pos, in_dim) in unlinked.iter() {
        let section = dimensions
            .get(in_dim.0)
            .ok()
            .and_then(|index| index.get(ChunkPos::from(pos.0)));
        let Some(section) = section else {
            tracing::debug!(pos = %pos.0, "a block entity outside any loaded section");
            commands.entity(entity).despawn();
            continue;
        };
        commands.entity(entity).insert(InSection(section));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::dimension::DimensionTypeConfig;
    use crate::world::storage::chunk::ChunkIndex;
    use bevy_app::{App, FixedUpdate};

    fn app_with_section() -> (App, Entity, Entity) {
        let mut app = App::new();
        app.add_systems(FixedUpdate, reconcile_block_entities);
        let dim = app
            .world_mut()
            .spawn((ChunkIndex::default(), DimensionTypeConfig::new(0, 256)))
            .id();
        let section = app
            .world_mut()
            .spawn((
                ChunkPos::new(0, 1, 0),
                InDimension(dim),
                SectionBlockEntities::default(),
            ))
            .id();
        app.world_mut()
            .get_mut::<ChunkIndex>(dim)
            .unwrap()
            .insert(ChunkPos::new(0, 1, 0), section);
        (app, dim, section)
    }

    #[test]
    fn the_link_and_the_index_are_attached_together() {
        let (mut app, dim, section) = app_with_section();
        let block_entity = app
            .world_mut()
            .spawn((BlockEntityPos(BlockPos::new(3, 20, 5)), InDimension(dim)))
            .id();
        app.world_mut().run_schedule(FixedUpdate);

        assert_eq!(
            app.world().get::<InSection>(block_entity).map(|l| l.0),
            Some(section)
        );
        assert_eq!(
            app.world()
                .get::<SectionBlockEntities>(section)
                .map(|held| held.to_vec()),
            Some(vec![block_entity])
        );
    }

    #[test]
    fn a_block_entity_without_a_section_leaves() {
        let (mut app, dim, _) = app_with_section();
        let orphan = app
            .world_mut()
            .spawn((BlockEntityPos(BlockPos::new(3, 200, 5)), InDimension(dim)))
            .id();
        app.world_mut().run_schedule(FixedUpdate);
        assert!(app.world().get_entity(orphan).is_err());
    }
}
