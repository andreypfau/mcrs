use bevy::prelude::{Commands, Component, Entity, Query, With};

#[derive(Component, Copy, Clone, Default, PartialEq, Eq, Debug)]
pub struct Despawned;

pub(super) fn despawn_marked_entities(
    entities: Query<Entity, With<Despawned>>,
    mut commands: Commands,
) {
    for entity in &entities {
        commands.entity(entity).despawn();
    }
}
