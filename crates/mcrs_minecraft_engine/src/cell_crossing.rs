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
use crate::loaded_client::UnloadedClient;
use crate::persisted_registry::CaptureCause;
use crate::player_dimension_change;
use crate::replicated::Replicated;

#[derive(Clone, Debug)]
pub struct Crossing {
    pub(crate) session: PlayerSession,
    pub(crate) carried: CarriedPlayer,
    pub(crate) replicated: Option<Replicated>,
    pub(crate) unloaded: Option<UnloadedClient>,
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
    let unloaded = leaving.take::<UnloadedClient>();
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
        unloaded,
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
        unloaded,
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
    match unloaded {
        Some(unloaded) => {
            arrived.insert(unloaded);
        }
        None => {
            arrived.remove::<UnloadedClient>();
        }
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_app::App;
    use mcrs_minecraft_engine_world::entity::EntityUuid;
    use mcrs_minecraft_engine_world::entity::player::Player;
    use mcrs_minecraft_engine_world::world::dimension::Dimension;
    use mcrs_minecraft_game_mode::PlayerGameMode;
    use mcrs_minecraft_protocol::uuid::Uuid;
    use mcrs_minecraft_swing::{SwingModifiers, SwingTracker};

    use crate::ability::{PlayerFlying, PlayerOpLevel};
    use crate::loaded_client::LoadedClientPlugin;
    use crate::persistence::PlayerPersistencePlugin;
    use crate::persistence::tests::profile;
    use crate::swing::{PlayerSwing, PlayerSwingPlugin};
    use crate::teleport::TeleportState;

    fn test_app() -> App {
        let mut app = App::new();
        app.add_plugins((
            PlayerPersistencePlugin,
            LoadedClientPlugin,
            PlayerSwingPlugin,
        ));
        app.world_mut().spawn(Dimension);
        app
    }

    fn spawn_crossing_player(world: &mut World, session: PlayerSession) -> Entity {
        world
            .spawn((
                Player,
                Owner(session),
                EntityUuid(Uuid::nil()),
                Transform::default(),
                PlayerGameMode::default(),
                PlayerFlying(false),
                PlayerOpLevel(1),
                TeleportState::default(),
                profile(),
            ))
            .id()
    }

    #[test]
    fn cell_crossing_preserves_loaded_state_and_swing_cooldown() {
        let mut app = test_app();
        let mut tracker = SwingTracker::default();
        tracker.try_start(40, 10, SwingModifiers::default());
        let expected_swing = PlayerSwing(tracker);

        let loaded_player = spawn_crossing_player(app.world_mut(), PlayerSession(1));
        app.world_mut()
            .entity_mut(loaded_player)
            .remove::<UnloadedClient>()
            .insert(expected_swing);
        let crossing = cross(app.world_mut(), loaded_player).expect("loaded player crosses");
        let arrived_loaded = arrive(app.world_mut(), crossing).expect("loaded player arrives");
        assert!(
            app.world().get::<UnloadedClient>(arrived_loaded).is_none(),
            "loaded client stays loaded across same-dimension cell crossing"
        );
        assert_eq!(
            app.world().get::<PlayerSwing>(arrived_loaded),
            Some(&expected_swing),
            "active swing cooldown survives same-dimension cell crossing"
        );

        let waiting_player = spawn_crossing_player(app.world_mut(), PlayerSession(2));
        {
            let mut waiting = app
                .world_mut()
                .get_mut::<UnloadedClient>(waiting_player)
                .expect("waiting player starts unloaded");
            for _ in 0..25 {
                assert!(!waiting.tick());
            }
        }
        let expected_waiting = *app
            .world()
            .get::<UnloadedClient>(waiting_player)
            .expect("waiting state");
        let crossing = cross(app.world_mut(), waiting_player).expect("waiting player crosses");
        let arrived_waiting = arrive(app.world_mut(), crossing).expect("waiting player arrives");
        assert_eq!(
            app.world().get::<UnloadedClient>(arrived_waiting),
            Some(&expected_waiting),
            "in-progress client load timer survives cell crossing without resetting"
        );
    }
}
