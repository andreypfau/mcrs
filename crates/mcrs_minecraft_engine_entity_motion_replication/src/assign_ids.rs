use bevy_ecs::entity::Entity;
use bevy_ecs::query::With;
use bevy_ecs::query::Without;
use bevy_ecs::resource::Resource;
use bevy_ecs::system::Commands;
use bevy_ecs::system::Query;
use bevy_ecs::system::ResMut;
use mcrs_minecraft_engine_entity_motion::MotionHold;
use mcrs_minecraft_engine_entity_motion::MotionState;
use mcrs_minecraft_engine_world::entity_ids::CellEntityIds;
use mcrs_minecraft_physics::EntityState;

use crate::Appearance;
use crate::MotionId;
use crate::subject::Subject;

#[derive(Resource, Debug, Default)]
pub(crate) struct Assigned(pub(crate) Vec<Release>);

#[derive(Clone, Copy, Debug)]
pub(crate) struct Release {
    pub(crate) subject: Subject,
    pub(crate) state: EntityState,
}

pub(crate) fn assign_ids(
    mut commands: Commands,
    pending: Query<(Entity, &MotionState), (With<Appearance>, Without<MotionId>)>,
    mut ids: ResMut<CellEntityIds>,
    mut assigned: ResMut<Assigned>,
) {
    for (entity, state) in &pending {
        let Some(id) = ids.allocate() else {
            tracing::warn!(?entity, "the cell has no entity id left; the entity waits");
            return;
        };
        commands
            .entity(entity)
            .try_insert(MotionId(id))
            .try_remove::<MotionHold>();
        assigned.0.push(Release {
            subject: Subject { id, entity },
            state: state.0,
        });
    }
}
