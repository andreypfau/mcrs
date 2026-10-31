use bevy_app::{App, Plugin, Update};
use bevy_ecs::change_detection::{Res, ResMut};
use bevy_ecs::entity::Entity;
use bevy_ecs::lifecycle::Remove;
use bevy_ecs::observer::On;
use bevy_ecs::prelude::{Query, Resource, resource_changed};
use bevy_ecs::query::{Changed, With};
use bevy_ecs::schedule::IntoScheduleConfigs;
use mcrs_minecraft_engine::session::GameProfile;
use mcrs_minecraft_network::{ServerSideConnection, StatusPlayers, StatusPublisher};
use mcrs_minecraft_protocol::ConnectionState;
use mcrs_minecraft_protocol::uuid::Uuid;

pub struct StatusPlugin;

impl Plugin for StatusPlugin {
    fn build(&self, app: &mut App) {
        app.add_observer(on_disconnect_handle);
        app.add_systems(
            Update,
            (
                on_connection_state_change_handle,
                publish_status_players.run_if(resource_changed::<OnlinePlayers>),
            )
                .chain(),
        );
        app.init_resource::<OnlinePlayers>();
    }
}

fn publish_status_players(players: Res<OnlinePlayers>, publisher: Res<StatusPublisher>) {
    let mut vec = Vec::new();
    for (_, uuid, name) in &players.players {
        vec.push((*uuid, name.clone()));
    }

    publisher.publish(StatusPlayers { players: vec });
}

#[derive(Resource, Default)]
struct OnlinePlayers {
    players: Vec<(Entity, Uuid, String)>,
}

fn on_disconnect_handle(
    trigger: On<Remove, ServerSideConnection>,
    mut players: ResMut<OnlinePlayers>,
) {
    let e = trigger.event().entity;
    players.players.retain(|x| x.0 != e);
}

fn on_connection_state_change_handle(
    query: Query<
        (Entity, &ConnectionState, &GameProfile),
        (Changed<ConnectionState>, With<ServerSideConnection>),
    >,
    mut players: ResMut<OnlinePlayers>,
) {
    for (entity, state, profile) in &query {
        if *state == ConnectionState::Game && !players.players.iter().any(|x| x.0 == entity) {
            players
                .players
                .push((entity, profile.id, profile.username.clone()));
        }
    }
}
