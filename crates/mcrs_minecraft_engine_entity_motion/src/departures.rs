use bevy_ecs::lifecycle::Remove;
use bevy_ecs::observer::On;
use bevy_ecs::resource::Resource;
use bevy_ecs::system::Commands;
use bevy_ecs::system::Query;
use bevy_ecs::system::ResMut;
use mcrs_minecraft_entity_key::EntityId;
use mcrs_minecraft_entity_key::TileKey;

use crate::MotionId;
use crate::MotionState;

/// An entity that lost its motion, with the tile of its last position.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct Departure {
    pub(crate) id: EntityId,
    pub(crate) tile: TileKey,
}

/// The entities that lost their motion since the last publish.
#[derive(Resource, Debug, Default)]
pub(crate) struct Departures(pub(crate) Vec<Departure>);

/// Keeps the departure of an entity that is despawned or loses its [`MotionState`], and takes
/// back its id, so that a new motion gets a new id and a new spawn.
pub(crate) fn depart(
    removed: On<Remove<MotionState>>,
    entities: Query<(&MotionState, &MotionId)>,
    mut departures: ResMut<Departures>,
    mut commands: Commands,
) {
    let Ok((state, id)) = entities.get(removed.entity) else {
        return;
    };
    departures.0.push(Departure {
        id: id.0,
        tile: TileKey::at(state.0.body.position),
    });
    commands.entity(removed.entity).try_remove::<MotionId>();
}
