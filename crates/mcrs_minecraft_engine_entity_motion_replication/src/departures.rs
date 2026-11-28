use bevy_ecs::lifecycle::Remove;
use bevy_ecs::observer::On;
use bevy_ecs::query::Has;
use bevy_ecs::resource::Resource;
use bevy_ecs::system::Commands;
use bevy_ecs::system::Query;
use bevy_ecs::system::ResMut;
use mcrs_minecraft_engine_entity_motion::MotionState;
use mcrs_minecraft_engine_physics::Mob;
use mcrs_minecraft_entity_key::EntityId;
use mcrs_minecraft_entity_key::TileKey;

use crate::MotionId;
use crate::PublishedTile;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct Departure {
    pub(crate) id: EntityId,
    pub(crate) tile: TileKey,
    pub(crate) kind: DepartureKind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum DepartureKind {
    Body,
    Mob,
}

#[derive(Resource, Debug, Default)]
pub(crate) struct Departures(pub(crate) Vec<Departure>);

pub(crate) fn depart(
    removed: On<Remove<MotionState>>,
    mut entities: Query<(&MotionId, &mut PublishedTile, Has<Mob>)>,
    mut departures: ResMut<Departures>,
    mut commands: Commands,
) {
    let Ok((id, mut published, mob)) = entities.get_mut(removed.entity) else {
        return;
    };
    if let Some(tile) = published.0.take() {
        let kind = if mob {
            DepartureKind::Mob
        } else {
            DepartureKind::Body
        };
        departures.0.push(Departure {
            id: id.0,
            tile,
            kind,
        });
    }
    commands.entity(removed.entity).try_remove::<MotionId>();
}
