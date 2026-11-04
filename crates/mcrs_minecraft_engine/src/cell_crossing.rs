//! A player that walks from one cell of a world into another. It moves at the end of a tick and
//! arrives at the start of the next one with its session and its wire id, so neither its client
//! nor its viewers see it leave.

use bevy_ecs::component::Component;
use bevy_ecs::entity::Entity;
use bevy_ecs::prelude::With;
use bevy_ecs::world::World;
use bevy_math::DVec3;
use mcrs_minecraft_engine_world::entity::physics::Transform;
use mcrs_minecraft_engine_world::entity::player::Player;
use mcrs_minecraft_engine_world::entity::player::chunk_view::SentChunkView;
use mcrs_minecraft_engine_world::owned_columns::OwnedColumns;
use mcrs_minecraft_engine_world::session::{DimPlayerIndex, Owner, PlayerSession};
use mcrs_minecraft_engine_world::world::dimension::Dimension;
use mcrs_voxel::ColumnPos;

use crate::departure::CarriedPlayer;
use crate::dimension_change::DimensionChangeError;
use crate::hot_ticket::{CellCrossing, HotTicket, RemoteTicket, RemoteTickets, TicketSquare};
use crate::player_dimension_change;
use crate::replicated::Replicated;

/// A player that left its cell at the end of a tick for the cell that owns the column it stands
/// in.
#[derive(Clone, Debug)]
pub struct Crossing {
    pub(crate) session: PlayerSession,
    pub(crate) carried: CarriedPlayer,
    pub(crate) replicated: Option<Replicated>,
    pub(crate) view: SentChunkView,
    pub(crate) ticket: Option<TicketSquare>,
}

impl Crossing {
    #[must_use]
    pub fn session(&self) -> PlayerSession {
        self.session
    }

    #[must_use]
    pub fn position(&self) -> DVec3 {
        self.carried.transform.translation
    }
}

/// On a player that arrived from another cell in this tick, with the transform it arrived
/// with: its viewers saw that pose already, so it publishes no move unless it moves again.
#[derive(Component, Clone, Copy, Debug)]
#[component(storage = "SparseSet")]
pub struct CrossedAt(pub(crate) Transform);

/// Lets go of each player that stands in a column that this cell does not own, into
/// `crossings`. The cell keeps the hot ticket of the player as a remote ticket, as the other
/// cells do, so the columns around the player stay hot in every cell.
pub(crate) fn cross_players(world: &mut World, crossings: &mut Vec<Crossing>) {
    let Some(owned) = world.get_resource::<OwnedColumns>().copied() else {
        return;
    };
    let leaving: Vec<Entity> = world
        .query_filtered::<(Entity, &Transform), (With<Player>, With<Owner>)>()
        .iter(world)
        .filter(|(_, transform)| !owned.owns(ColumnPos::from(transform.translation)))
        .map(|(player, _)| player)
        .collect();
    for player in leaving {
        match cross(world, player) {
            Ok(crossing) => crossings.push(crossing),
            Err(error) => tracing::error!(?player, "a player did not cross to its cell: {error}"),
        }
    }
}

fn cross(world: &mut World, player: Entity) -> Result<Crossing, DimensionChangeError> {
    let mut leaving = world
        .get_entity_mut(player)
        .map_err(|_| DimensionChangeError::NoPlayer)?;
    let Some(&Owner(session)) = leaving.get::<Owner>() else {
        return Err(DimensionChangeError::NoPlayer);
    };
    leaving.insert(CellCrossing);
    let carried = match CarriedPlayer::take(&mut leaving) {
        Ok(carried) => carried,
        Err(error) => {
            leaving.remove::<CellCrossing>();
            return Err(error);
        }
    };
    let replicated = leaving.take::<Replicated>();
    let view = leaving.take::<SentChunkView>().unwrap_or_default();
    let ticket = leaving.take::<HotTicket>();
    world.despawn(player);
    if let Some(ticket) = ticket {
        hold_remote(world, session, ticket);
    }
    Ok(Crossing {
        session,
        carried,
        replicated,
        view,
        ticket: ticket.map(HotTicket::square),
    })
}

/// Spawns the player of `crossing` where it stood, with what its viewers and its client know
/// of it. Its remote ticket here becomes its own.
pub(crate) fn arrive(
    world: &mut World,
    crossing: Crossing,
) -> Result<Entity, DimensionChangeError> {
    let Crossing {
        session,
        carried,
        replicated,
        view,
        ticket,
    } = crossing;
    release_remote(world, session);
    let transform = carried.transform;
    let player = player_dimension_change::place(world, session, transform, carried, view)?;
    let dimension = own_dimension(world)?;
    let mut arrived = world.entity_mut(player);
    if let Some(replicated) = replicated {
        arrived.insert((replicated, CrossedAt(transform)));
    }
    if let Some(square) = ticket {
        arrived.insert(HotTicket::in_dim(dimension, square));
    }
    Ok(player)
}

/// Holds `square` for the player of `session` of another cell, or lets its ticket go. A player
/// of this cell keeps its own ticket.
pub(crate) fn update_remote(
    world: &mut World,
    session: PlayerSession,
    square: Option<TicketSquare>,
) {
    let own = world
        .get_resource::<DimPlayerIndex>()
        .is_some_and(|index| index.0.contains_key(&session));
    if own {
        return;
    }
    release_remote(world, session);
    let (Some(square), Ok(dimension)) = (square, own_dimension(world)) else {
        return;
    };
    hold_remote(world, session, HotTicket::in_dim(dimension, square));
}

fn hold_remote(world: &mut World, session: PlayerSession, ticket: HotTicket) {
    let holder = world.spawn((RemoteTicket, ticket)).id();
    world
        .get_resource_or_init::<RemoteTickets>()
        .0
        .insert(session, holder);
}

fn release_remote(world: &mut World, session: PlayerSession) {
    let held = world
        .get_resource_mut::<RemoteTickets>()
        .and_then(|mut tickets| tickets.0.remove(&session));
    if let Some(holder) = held {
        world.despawn(holder);
    }
}

fn own_dimension(world: &mut World) -> Result<Entity, DimensionChangeError> {
    world
        .query_filtered::<Entity, With<Dimension>>()
        .single(world)
        .map_err(|_| DimensionChangeError::NoDimension)
}
