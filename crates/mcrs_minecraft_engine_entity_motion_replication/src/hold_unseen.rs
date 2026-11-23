use bevy_ecs::lifecycle::Add;
use bevy_ecs::observer::On;
use bevy_ecs::query::With;
use bevy_ecs::system::Commands;
use bevy_ecs::system::Query;
use mcrs_minecraft_engine_entity_motion::MotionHold;
use mcrs_minecraft_engine_entity_motion::MotionState;

use crate::MotionId;
use crate::PublishedTile;

pub(crate) fn hold_unseen(
    added: On<Add<MotionState>>,
    numbered: Query<(), With<MotionId>>,
    mut commands: Commands,
) {
    let mut entity = commands.entity(added.entity);
    if numbered.contains(added.entity) {
        entity.try_insert(PublishedTile::default());
    } else {
        entity.try_insert((MotionHold, PublishedTile::default()));
    }
}
