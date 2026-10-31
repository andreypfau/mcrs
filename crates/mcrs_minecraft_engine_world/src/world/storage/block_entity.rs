use bevy_derive::Deref;
use bevy_ecs::prelude::{Commands, Component, Entity, Query, Without};
use mcrs_voxel::{BlockPos, SectionPos};

use crate::world::dimension::InDimension;
use crate::world::storage::section::SectionIndex;
use mcrs_minecraft_block_entity::GeneratedBlockEntity;

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

/// A block entity's kind and state, in the one typed shape the save, the chunk
/// packet and the anvil reader share; the position is read off the entity's
/// [`BlockEntityPos`] and duplicated here only because the shape carries it.
#[derive(Component, Clone, Debug, PartialEq)]
pub struct BlockEntity(pub GeneratedBlockEntity);

/// One entity per entry, under the dimension whose sections the reconcile step
/// resolves them against.
pub fn spawn_block_entities(
    commands: &mut Commands,
    dim: InDimension,
    entries: Vec<GeneratedBlockEntity>,
) {
    for entry in entries {
        let pos = entry.position();
        commands.spawn((
            BlockEntityPos(BlockPos::new(pos.x, pos.y, pos.z)),
            dim,
            BlockEntity(entry),
        ));
    }
}

/// A block entity whose section is not loaded has nowhere to live and leaves
/// again: the section is what owns it, and a section that arrives later brings
/// its own from the save or the generator.
pub fn reconcile_block_entities(
    unlinked: Query<(Entity, &BlockEntityPos, &InDimension), Without<InSection>>,
    dimensions: Query<&SectionIndex>,
    mut commands: Commands,
) {
    for (entity, pos, in_dim) in unlinked.iter() {
        let section = dimensions
            .get(in_dim.0)
            .ok()
            .and_then(|index| index.get(SectionPos::from(pos.0)));
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
    use crate::world::storage::section::SectionIndex;
    use bevy_app::{App, FixedUpdate};

    fn app_with_section() -> (App, Entity, Entity) {
        let mut app = App::new();
        app.add_systems(FixedUpdate, reconcile_block_entities);
        let dim = app
            .world_mut()
            .spawn((SectionIndex::default(), DimensionTypeConfig::new(0, 256)))
            .id();
        let section = app
            .world_mut()
            .spawn((
                SectionPos::new(0, 1, 0),
                InDimension(dim),
                SectionBlockEntities::default(),
            ))
            .id();
        app.world_mut()
            .get_mut::<SectionIndex>(dim)
            .unwrap()
            .insert(SectionPos::new(0, 1, 0), section);
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
