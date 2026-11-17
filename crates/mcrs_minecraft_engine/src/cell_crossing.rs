use bevy_ecs::component::Component;
use bevy_ecs::entity::Entity;
use bevy_ecs::prelude::With;
use bevy_ecs::world::World;
use mcrs_minecraft_engine_world::entity::physics::Transform;
use mcrs_minecraft_engine_world::entity::player::chunk_view::SentChunkView;
use mcrs_minecraft_engine_world::owned_columns::OwnedColumns;
use mcrs_minecraft_engine_world::session::{DimPlayerIndex, Owner, PlayerSession};
use mcrs_minecraft_engine_world::world::dimension::InDimension;
use mcrs_minecraft_engine_world::world::lifecycle::simulation::{
    HeldSimulationArea, SimulationTicket,
};
use mcrs_voxel::ColumnPos;

use crate::departure::{Carriable, CarriedPlayer};
use crate::dimension_change::DimensionChangeError;
use crate::hot_ticket;
use crate::hot_ticket::{CellCrossing, HotTicket, RemoteTicket, RemoteTickets};
use crate::persisted_registry::CaptureCause;
use crate::player_dimension_change;
use crate::replicated::Replicated;

#[derive(Clone, Debug)]
pub struct Crossing {
    pub(crate) session: PlayerSession,
    pub(crate) carried: CarriedPlayer,
    pub(crate) replicated: Option<Replicated>,
    pub(crate) view: SentChunkView,
    pub(crate) ticket: Option<HotTicket>,
}

#[derive(Component, Clone, Copy, Debug)]
#[component(storage = "SparseSet")]
pub struct CrossedAt(pub(crate) Transform);

pub(crate) fn cross_players(world: &mut World, crossings: &mut Vec<Crossing>) {
    let Some(owned) = world.get_resource::<OwnedColumns>().copied() else {
        return;
    };
    let leaving: Vec<Entity> = world
        .query_filtered::<(Entity, &Transform), (Carriable, With<Owner>)>()
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
    let carried = match CarriedPlayer::take(&mut leaving, CaptureCause::CellCrossing) {
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
        ticket,
    })
}

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
    let transform = carried.transform();
    let player = player_dimension_change::place(world, session, transform, carried, view)?;
    release_remote(world, session);
    let mut arrived = world.entity_mut(player);
    let Some(&InDimension(dimension)) = arrived.get::<InDimension>() else {
        return Err(DimensionChangeError::NoDimension);
    };
    if let Some(replicated) = replicated {
        arrived.insert((replicated, CrossedAt(transform)));
    }
    if let Some(carried) = ticket {
        arrived.insert((
            carried,
            SimulationTicket::new(dimension, carried.simulation),
        ));
    }
    Ok(player)
}

pub(crate) fn update_remote(world: &mut World, session: PlayerSession, ticket: Option<HotTicket>) {
    let own = world
        .get_resource::<DimPlayerIndex>()
        .is_some_and(|index| index.0.contains_key(&session));
    if own {
        return;
    }
    let held = world
        .get_resource::<RemoteTickets>()
        .and_then(|tickets| tickets.0.get(&session).copied());
    match (ticket, held) {
        (Some(ticket), Some(holder)) => hot_ticket::move_remote_ticket(world, holder, ticket),
        (Some(ticket), None) => {
            hold_remote(world, session, ticket);
        }
        (None, _) => release_remote(world, session),
    }
}

fn hold_remote(world: &mut World, session: PlayerSession, ticket: HotTicket) {
    let Ok(dimension) = player_dimension_change::own_dimension(world) else {
        return;
    };
    let holder = world
        .spawn((
            RemoteTicket,
            ticket,
            SimulationTicket::new(dimension, ticket.simulation),
            HeldSimulationArea::new(dimension, ticket.simulation),
        ))
        .id();
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
