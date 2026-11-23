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
use mcrs_minecraft_game_mode::{GameMode, PlayerGameMode};
use mcrs_minecraft_resource::ResourceKeyBuf;

use crate::bus::{self, PacketPayload};
use crate::coordinator::clock::GameTime;
use crate::dimension_change::{Arrival, DimensionChangeError};
use crate::teleport::TeleportState;

#[derive(Clone, Debug, PartialEq)]
pub enum PlayerOrder {
    Teleport(TeleportOrder),
    SetGameMode(GameMode),
}

#[derive(Clone, Debug, PartialEq)]
pub struct TeleportOrder {
    pub to: ResourceKeyBuf<DimensionKey>,
    pub arrival: Arrival,
}

#[derive(Message, Clone, Debug)]
pub struct OrderedPlayer {
    pub session: PlayerSession,
    pub order: PlayerOrder,
}

#[derive(Resource, Default)]
pub struct DepartureRequests(Vec<DepartureRequest>);

pub(crate) struct DepartureRequest {
    pub(crate) player: Entity,
    pub(crate) to: ResourceKeyBuf<DimensionKey>,
    pub(crate) arrival: Arrival,
}

impl DepartureRequests {
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
    let now = world
        .get_resource::<GameTime>()
        .copied()
        .unwrap_or_default();
    let mut entity = world
        .get_entity_mut(player)
        .map_err(|_| DimensionChangeError::NoPlayer)?;
    let Some(&Owner(session)) = entity.get::<Owner>() else {
        return Err(DimensionChangeError::NoPlayer);
    };
    let Some(mut transform) = entity.get_mut::<Transform>() else {
        return Err(DimensionChangeError::NoPlayer);
    };
    transform.set_if_neq(arrival.transform);
    let teleport_id = entity
        .get_mut::<TeleportState>()
        .map(|mut state| state.sent(arrival.transform, now))
        .ok_or(DimensionChangeError::NoArrivalState)?;
    let position = arrival.position_packet(teleport_id);
    world.write_message(bus::to(session, PacketPayload::PlayerPosition(position)));
    if let Some(look_at) = arrival.look_at {
        world.write_message(bus::to(session, PacketPayload::PlayerLookAt(look_at)));
    }
    Ok(())
}

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

#[cfg_attr(
    mcrs_lint,
    expect(
        mcrs::exclusive_system,
        reason = "an order goes through the world-level player API, which a command and a plugin also call"
    )
)]
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
