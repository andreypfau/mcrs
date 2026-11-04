//! What a command asks of a player: applied at once when the world of the command holds the
//! player, and sent to the world that holds it otherwise.

use bevy_ecs::change_detection::DetectChangesMut;
use bevy_ecs::entity::Entity;
use bevy_ecs::message::{Message, Messages};
use bevy_ecs::prelude::With;
use bevy_ecs::resource::Resource;
use bevy_ecs::world::World;
use mcrs_minecraft_dimension::Dimension as DimensionKey;
use mcrs_minecraft_engine_world::entity::physics::Transform;
use mcrs_minecraft_engine_world::session::{DimPlayerIndex, Owner, PlayerSession};
use mcrs_minecraft_engine_world::world::dimension::Dimension;
use mcrs_minecraft_game_mode::GameMode;
use mcrs_minecraft_resource::ResourceKeyBuf;

use crate::ability::PlayerGameMode;
use crate::bus::{self, PacketPayload};
use crate::dimension_change::{Arrival, DimensionChangeError};
use crate::teleport::TeleportState;

/// A change to one player that a command makes.
#[derive(Clone, Debug, PartialEq)]
pub enum PlayerOrder {
    /// The player moves, to another dimension when `to` names one.
    Teleport(TeleportOrder),
    /// The player gets the game mode.
    SetGameMode(GameMode),
}

/// Where a teleport sends a player.
#[derive(Clone, Debug, PartialEq)]
pub struct TeleportOrder {
    /// The dimension the player is in after the move.
    pub to: ResourceKeyBuf<DimensionKey>,
    /// The place and the packets of the arrival.
    pub arrival: Arrival,
}

/// An order for the player of `session`, for the world that holds it.
#[derive(Message, Clone, Debug)]
pub struct OrderedPlayer {
    pub session: PlayerSession,
    pub order: PlayerOrder,
}

/// The players that the orders of this tick send to another dimension. The world lets each go
/// after the tick, as it lets go a player that a portal sends; until then the player stays here.
#[derive(Resource, Default)]
pub struct DepartureRequests(Vec<DepartureRequest>);

pub(crate) struct DepartureRequest {
    pub(crate) player: Entity,
    pub(crate) to: ResourceKeyBuf<DimensionKey>,
    pub(crate) arrival: Arrival,
}

impl DepartureRequests {
    /// The dimension that `player` leaves for after this tick.
    pub fn destination(&self, player: Entity) -> Option<&ResourceKeyBuf<DimensionKey>> {
        self.0
            .iter()
            .find(|request| request.player == player)
            .map(|request| &request.to)
    }

    fn set(&mut self, request: DepartureRequest) {
        self.cancel(request.player);
        self.0.push(request);
    }

    fn cancel(&mut self, player: Entity) {
        self.0.retain(|request| request.player != player);
    }

    pub(crate) fn take(&mut self) -> Vec<DepartureRequest> {
        std::mem::take(&mut self.0)
    }
}

/// Applies `order` to `player`, an entity of `world`.
pub fn obey(
    world: &mut World,
    player: Entity,
    order: PlayerOrder,
) -> Result<(), DimensionChangeError> {
    match order {
        PlayerOrder::Teleport(order) => teleport(world, player, order),
        PlayerOrder::SetGameMode(mode) => set_game_mode(world, player, mode),
    }
}

/// Moves `player` in this world at once, or asks this world to let it go to the dimension of
/// `order` after the tick. The last order of a tick wins: a move here drops a departure that an
/// earlier order asked for.
pub fn teleport(
    world: &mut World,
    player: Entity,
    TeleportOrder { to, arrival }: TeleportOrder,
) -> Result<(), DimensionChangeError> {
    let here = world
        .query_filtered::<&ResourceKeyBuf<DimensionKey>, With<Dimension>>()
        .single(world)
        .map_err(|_| DimensionChangeError::NoDimension)?
        == &to;
    if !here {
        world
            .get_resource_or_init::<DepartureRequests>()
            .set(DepartureRequest {
                player,
                to,
                arrival,
            });
        return Ok(());
    }
    if let Some(mut requests) = world.get_resource_mut::<DepartureRequests>() {
        requests.cancel(player);
    }
    let mut entity = world
        .get_entity_mut(player)
        .map_err(|_| DimensionChangeError::NoPlayer)?;
    let Some(&Owner(session)) = entity.get::<Owner>() else {
        return Err(DimensionChangeError::NoPlayer);
    };
    let Some(mut transform) = entity.get_mut::<Transform>() else {
        return Err(DimensionChangeError::NoPlayer);
    };
    *transform = arrival.transform;
    let teleport_id = entity
        .get_mut::<TeleportState>()
        .map(|mut state| state.sent(arrival.transform))
        .ok_or(DimensionChangeError::NoArrivalState)?;
    let position = arrival.position_packet(teleport_id);
    world.write_message(bus::to(session, PacketPayload::PlayerPosition(position)).critical());
    if let Some(look_at) = arrival.look_at {
        world.write_message(bus::to(session, PacketPayload::PlayerLookAt(look_at)).critical());
    }
    Ok(())
}

/// Sets the game mode of `player`. Its world sends the packets of the change.
pub fn set_game_mode(
    world: &mut World,
    player: Entity,
    mode: GameMode,
) -> Result<(), DimensionChangeError> {
    let mut current = world
        .get_mut::<PlayerGameMode>(player)
        .ok_or(DimensionChangeError::NoPlayer)?;
    current.set_if_neq(PlayerGameMode(mode));
    Ok(())
}

/// Applies the orders that other worlds sent for the players of this world, in the order they
/// came, after the players that arrive in this tick are in the world.
pub(crate) fn obey_orders(world: &mut World) {
    let Some(mut written) = world.get_resource_mut::<Messages<OrderedPlayer>>() else {
        return;
    };
    let orders: Vec<OrderedPlayer> = written.drain().collect();
    for OrderedPlayer { session, order } in orders {
        let player = world
            .get_resource::<DimPlayerIndex>()
            .and_then(|index| index.0.get(&session).copied());
        let Some(player) = player else {
            tracing::error!(?session, "an order names a player this world does not hold");
            continue;
        };
        if let Err(error) = obey(world, player, order) {
            tracing::error!(?session, "an order did not apply: {error}");
        }
    }
}
