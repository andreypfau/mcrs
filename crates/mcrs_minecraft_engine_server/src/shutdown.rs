use bevy_app::{App, AppExit, Plugin, Update};
use bevy_ecs::change_detection::Res;
use bevy_ecs::entity::Entity;
use bevy_ecs::message::{Message, MessageReader, MessageWriter};
use bevy_ecs::query::With;
use bevy_ecs::system::{Commands, Local, Query};
use mcrs_minecraft_engine::disconnect::{Departing, PendingDisconnectQueue};
use mcrs_minecraft_network::ServerSideConnection;
use mcrs_minecraft_protocol::ConnectionState;
use mcrs_minecraft_text::Text;

use crate::login::LoginState;

/// Asks the server to disconnect every connection, including those that arrive later, and to
/// write [`AppExit::Success`] once no connection, queued disconnect or unsaved player is left.
#[derive(Message, Clone, Copy, Debug, PartialEq, Eq)]
pub struct ShutdownRequest;

/// Handles [`ShutdownRequest`]. It installs no signal handler; the embedder writes the request.
pub struct ShutdownPlugin;

impl Plugin for ShutdownPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<ShutdownRequest>();
        app.add_systems(Update, shutdown_system);
    }
}

pub(crate) fn shutdown_system(
    requests: MessageReader<ShutdownRequest>,
    mut shutting_down: Local<bool>,
    mut connections: Query<(
        Entity,
        &mut ServerSideConnection,
        &ConnectionState,
        Option<&LoginState>,
    )>,
    pending: Res<PendingDisconnectQueue>,
    departing: Query<(), With<Departing>>,
    mut commands: Commands,
    mut exit: MessageWriter<AppExit>,
) {
    if !requests.is_empty() {
        *shutting_down = true;
    }
    if !*shutting_down {
        return;
    }

    for (entity, mut side_connection, &connection_state, login) in connections.iter_mut() {
        crate::login::disconnect(
            &mut side_connection,
            crate::login::client_protocol(connection_state, login),
            Text::translate("multiplayer.disconnect.server_shutdown", Vec::new()),
        );
        commands.entity(entity).remove::<ServerSideConnection>();
    }

    if connections.is_empty() && pending.entries.is_empty() && departing.is_empty() {
        exit.write(AppExit::Success);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mcrs_minecraft_engine_world::session::PlayerSession;
    use mcrs_minecraft_protocol::uuid::Uuid;

    #[test]
    fn a_shutdown_request_exits_the_app() {
        let mut app = App::new();
        app.init_resource::<PendingDisconnectQueue>();
        app.add_plugins(ShutdownPlugin);
        app.world_mut().write_message(ShutdownRequest);
        app.update();
        assert_eq!(app.should_exit(), Some(AppExit::Success));
    }

    #[test]
    fn the_app_waits_for_a_departing_player() {
        let mut app = App::new();
        app.init_resource::<PendingDisconnectQueue>();
        app.add_plugins(ShutdownPlugin);
        let player = app
            .world_mut()
            .spawn(Departing {
                id: Uuid::from_u128(1),
                dim: Entity::PLACEHOLDER,
                session: PlayerSession(1),
            })
            .id();
        app.world_mut().write_message(ShutdownRequest);

        app.update();
        assert_eq!(app.should_exit(), None);

        app.world_mut().despawn(player);
        app.update();
        assert_eq!(app.should_exit(), Some(AppExit::Success));
    }
}
