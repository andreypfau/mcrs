use bevy_ecs::entity::Entity;
use bevy_ecs::message::Messages;
use bevy_ecs::prelude::With;
use bevy_ecs::world::World;
use mcrs_engine_muxer::EntityEvent;
use mcrs_minecraft_dimension::Dimension as DimensionKey;
use mcrs_minecraft_engine_world::entity::physics::Transform;
use mcrs_minecraft_engine_world::entity::player::chunk_view::SentChunkView;
use mcrs_minecraft_engine_world::entity::player::{ChangeDimensionRequest, Player};
use mcrs_minecraft_engine_world::session::{Owner, PlayerSession};
use mcrs_minecraft_engine_world::world::dimension::{Dimension, InDimension};
use mcrs_minecraft_replication::PlayerData;
use mcrs_minecraft_resource::ResourceKeyBuf;

use crate::ability::OpLevelAnnounced;
use crate::departure::{CarriedPlayer, Departure, PlayerTransfer};
use crate::dimension_change::{Arrival, DimensionChangeError};
use crate::game_mode::ListedGameMode;
use crate::minecraft_entity::EntityBundle;
use crate::player_order::{DepartureRequest, DepartureRequests};
use crate::replicated::Replicated;
use crate::tile_events::TileEvents;

/// Lets go of each player that a [`ChangeDimensionRequest`] of this tick names, in the order of
/// writing, then of each player that a command sends away, into `departures`. A request that
/// this world cannot meet leaves the player where it is.
pub(crate) fn depart_players(world: &mut World, departures: &mut Vec<Departure>) {
    let ordered = world
        .get_resource_mut::<DepartureRequests>()
        .map(|mut requests| requests.take())
        .unwrap_or_default();
    let requests: Vec<ChangeDimensionRequest> = world
        .get_resource_mut::<Messages<ChangeDimensionRequest>>()
        .map(|mut written| written.drain().collect())
        .unwrap_or_default();
    if requests.is_empty() && ordered.is_empty() {
        return;
    }
    let own = world
        .query_filtered::<&ResourceKeyBuf<DimensionKey>, With<Dimension>>()
        .single(world)
        .ok()
        .cloned();
    for request in requests {
        if own.as_ref() == Some(&request.to) {
            tracing::error!(to = %request.to, "a dimension change names its own dimension");
            continue;
        }
        let arrival = Arrival::absolute(request.transform);
        match depart(world, request.player, request.to, arrival) {
            Ok(departure) => departures.push(departure),
            Err(error) => {
                tracing::error!(player = ?request.player, "a player did not change dimension: {error}");
            }
        }
    }
    for DepartureRequest {
        player,
        to,
        arrival,
    } in ordered
    {
        match depart(world, player, to, arrival) {
            Ok(departure) => departures.push(departure),
            Err(error) => {
                tracing::error!(?player, "a player did not change dimension: {error}");
            }
        }
    }
}

/// Takes the carried state of `player` and despawns it. Its viewers here lose it.
fn depart(
    world: &mut World,
    player: Entity,
    to: ResourceKeyBuf<DimensionKey>,
    arrival: Arrival,
) -> Result<Departure, DimensionChangeError> {
    let mut leaving = world
        .get_entity_mut(player)
        .map_err(|_| DimensionChangeError::NoPlayer)?;
    let Some(&Owner(session)) = leaving.get::<Owner>() else {
        return Err(DimensionChangeError::NoPlayer);
    };
    let listed = leaving
        .get::<Replicated>()
        .zip(leaving.get::<ListedGameMode>())
        .filter(|(replicated, listed)| replicated.game_mode != listed.0)
        .map(|(replicated, listed)| (replicated.tile, replicated.id, listed.0));
    let carried = CarriedPlayer::take(&mut leaving)?;
    // A game mode that changed in the tick the player leaves reaches its viewers before its
    // despawn, as the publish of the next tick would have sent it.
    if let Some((tile, id, mode)) = listed
        && let Some(mut events) = world.get_resource_mut::<TileEvents>()
    {
        events.push(tile, id, EntityEvent::Data(PlayerData::GameMode(mode)));
    }
    world.despawn(player);
    Ok(Departure {
        session,
        to,
        arrival,
        carried,
    })
}

/// Spawns the players that the host handed this world in the input of this tick, before their
/// serverbound packets of the same tick reach them.
pub(crate) fn receive_transfers(world: &mut World) {
    let Some(mut messages) = world.get_resource_mut::<Messages<PlayerTransfer>>() else {
        return;
    };
    let transfers: Vec<PlayerTransfer> = messages.drain().collect();
    for transfer in transfers {
        match transfer {
            PlayerTransfer::Arrive(departure) => {
                if let Err(error) = arrive(world, departure) {
                    tracing::error!("a player did not arrive: {error}");
                }
            }
            PlayerTransfer::Return(departure) => {
                let transform = departure.carried.transform;
                if let Err(error) = place(
                    world,
                    departure.session,
                    transform,
                    departure.carried,
                    SentChunkView::default(),
                ) {
                    tracing::error!("a player did not return: {error}");
                }
            }
        }
    }
}

/// Spawns the player of `departure` at its arrival. Its view starts empty and fills as on a
/// join.
fn arrive(world: &mut World, departure: Departure) -> Result<Entity, DimensionChangeError> {
    let Departure {
        session,
        arrival,
        carried,
        ..
    } = departure;
    let level = carried.op_level;
    let arrived = place(
        world,
        session,
        arrival.transform,
        carried,
        SentChunkView::default(),
    )?;
    world.write_message(OpLevelAnnounced {
        player: arrived,
        session,
        level,
    });
    Ok(arrived)
}

/// Spawns the player of `session` with `carried`, and with `view` as the view its client holds.
pub(crate) fn place(
    world: &mut World,
    session: PlayerSession,
    transform: Transform,
    carried: CarriedPlayer,
    view: SentChunkView,
) -> Result<Entity, DimensionChangeError> {
    let dimension = own_dimension(world)?;
    let mut player = world.spawn((
        EntityBundle {
            minecraft_entity: Default::default(),
            dimension: InDimension(dimension),
            transform,
            uuid: carried.uuid,
        },
        Owner(session),
        Player,
        view,
    ));
    carried.insert_into(&mut player);
    Ok(player.id())
}

/// The dimension entity of this world.
pub(crate) fn own_dimension(world: &mut World) -> Result<Entity, DimensionChangeError> {
    world
        .query_filtered::<Entity, With<Dimension>>()
        .single(world)
        .map_err(|_| DimensionChangeError::NoDimension)
}
