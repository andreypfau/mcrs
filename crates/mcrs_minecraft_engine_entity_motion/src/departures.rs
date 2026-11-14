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
use crate::PublishedTile;

/// An entity that lost its motion, with the tile in which its viewers know it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct Departure {
    pub(crate) id: EntityId,
    pub(crate) tile: TileKey,
}

/// The entities that lost their motion since the last publish.
#[derive(Resource, Debug, Default)]
pub(crate) struct Departures(pub(crate) Vec<Departure>);

/// Keeps the departure of an entity that is despawned or loses its [`MotionState`], and takes
/// back its id, so that a new motion gets a new id and a new spawn. An entity whose spawn is not
/// published yet departs unseen.
pub(crate) fn depart(
    removed: On<Remove<MotionState>>,
    mut entities: Query<(&MotionId, &mut PublishedTile)>,
    mut departures: ResMut<Departures>,
    mut commands: Commands,
) {
    let Ok((id, mut published)) = entities.get_mut(removed.entity) else {
        return;
    };
    if let Some(tile) = published.0.take() {
        departures.0.push(Departure { id: id.0, tile });
    }
    commands.entity(removed.entity).try_remove::<MotionId>();
}
