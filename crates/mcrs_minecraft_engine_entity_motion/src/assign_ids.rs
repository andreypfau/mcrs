use bevy_ecs::entity::Entity;
use bevy_ecs::query::With;
use bevy_ecs::query::Without;
use bevy_ecs::resource::Resource;
use bevy_ecs::system::Commands;
use bevy_ecs::system::Query;
use bevy_ecs::system::ResMut;
use mcrs_minecraft_engine_world::entity_ids::CellEntityIds;

use crate::Appearance;
use crate::MotionId;
use crate::MotionState;
use crate::subject::Subject;

/// The entities that got their id in this tick and have not published their spawn yet.
#[derive(Resource, Debug, Default)]
pub(crate) struct Assigned(pub(crate) Vec<Subject>);

/// Gives each entity with a motion, an appearance and no id the next id of the cell, so that no
/// entity moves before its spawn. When the cell has no id left, the entity waits for the next
/// tick.
pub(crate) fn assign_ids(
    mut commands: Commands,
    pending: Query<Entity, (With<MotionState>, With<Appearance>, Without<MotionId>)>,
    mut ids: ResMut<CellEntityIds>,
    mut assigned: ResMut<Assigned>,
) {
    for entity in &pending {
        let Some(id) = ids.allocate() else {
            tracing::warn!(?entity, "the cell has no entity id left; the entity waits");
            return;
        };
        commands.entity(entity).try_insert(MotionId(id));
        assigned.0.push(Subject { id, entity });
    }
}
