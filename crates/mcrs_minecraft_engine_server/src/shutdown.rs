use bevy_app::{App, AppExit, Plugin, Update};
use bevy_ecs::entity::Entity;
use bevy_ecs::message::{Message, MessageReader, MessageWriter};
use bevy_ecs::query::With;
use bevy_ecs::system::{Commands, Local, Query, ResMut};
use mcrs_minecraft_engine::coordinator::dim_cells::DimCells;
use mcrs_minecraft_engine::muxer::disconnect::Departing;
use mcrs_minecraft_engine::{WorldStateSources, WorldStateWriter};
use mcrs_minecraft_network::ServerSideConnection;
use mcrs_minecraft_protocol::ConnectionState;
use mcrs_minecraft_text::Text;

use crate::login::LoginState;

/// Asks the server to disconnect every connection, including those that arrive later, and to
/// write [`AppExit::Success`] once no connection or unsaved player is left and the host files are
/// on the disk. A save that fails writes [`AppExit::error`] instead.
#[derive(Message, Clone, Copy, Debug, PartialEq, Eq)]
pub struct ShutdownRequest;

/// Handles [`ShutdownRequest`]. It installs no signal handler; the embedder writes the request.
pub struct ShutdownPlugin;

impl Plugin for ShutdownPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<ShutdownRequest>();
        app.init_resource::<WorldStateWriter>();
        app.add_systems(Update, shutdown_system);
    }
}

/// How far the shutdown has come. One value per server, so the steps cannot disagree.
#[derive(Default)]
pub(crate) enum ShutdownPhase {
    #[default]
    Running,
    /// The connections close, the dimensions save and the departing players reach the disk.
    Draining,
    /// The host files are on their way to the disk. `failed` holds the result of the save before.
    WritingHostState {
        failed: bool,
    },
    Done,
}

#[expect(
    clippy::too_many_arguments,
    reason = "a Bevy system: each message, query and resource is its own parameter so the scheduler sees the access"
)]
pub(crate) fn shutdown_system(
    requests: MessageReader<ShutdownRequest>,
    mut phase: Local<ShutdownPhase>,
    mut connections: Query<(
        Entity,
        &mut ServerSideConnection,
        &ConnectionState,
        Option<&LoginState>,
    )>,
    departing: Query<(), With<Departing>>,
    mut commands: Commands,
    mut cells: Option<ResMut<DimCells>>,
    sources: WorldStateSources,
    mut writer: ResMut<WorldStateWriter>,
    mut exit: MessageWriter<AppExit>,
) {
    if matches!(*phase, ShutdownPhase::Running) && !requests.is_empty() {
        if let Some(cells) = &mut cells {
            cells.request_save_all();
        }
        *phase = ShutdownPhase::Draining;
    }
    if matches!(*phase, ShutdownPhase::Draining) {
        for (entity, mut side_connection, &connection_state, login) in connections.iter_mut() {
            crate::login::disconnect(
                &mut side_connection,
                crate::login::client_protocol(connection_state, login),
                Text::translate("multiplayer.disconnect.server_shutdown", Vec::new()),
            );
            commands.entity(entity).remove::<ServerSideConnection>();
        }
        let failed = cells.as_ref().is_some_and(|cells| cells.save_failed());
        let saved = cells.as_ref().is_none_or(|cells| cells.save_complete());
        if connections.is_empty() && departing.is_empty() && (saved || failed) {
            if let Some(write) = sources.snapshot() {
                writer.submit(write);
            }
            *phase = ShutdownPhase::WritingHostState { failed };
        }
    }
    if let ShutdownPhase::WritingHostState { failed } = *phase {
        writer.reap();
        if writer.is_idle() {
            exit.write(if failed || writer.failed() {
                AppExit::error()
            } else {
                AppExit::Success
            });
            *phase = ShutdownPhase::Done;
        }
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
        app.add_plugins(ShutdownPlugin);
        app.world_mut().write_message(ShutdownRequest);
        app.update();
        assert_eq!(app.should_exit(), Some(AppExit::Success));
    }

    #[test]
    fn the_last_shutdown_writes_the_host_state() {
        let dir = std::env::temp_dir().join(format!("mcrs-shutdown-world-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut app = App::new();
        app.add_plugins(ShutdownPlugin);
        app.insert_resource(mcrs_minecraft_engine_world::world::world_save::WorldSave(
            dir.clone(),
        ));
        app.world_mut().write_message(ShutdownRequest);
        app.update();
        assert_eq!(app.should_exit(), Some(AppExit::Success));
        let level = mcrs_minecraft_save::read_level_dat(&dir).unwrap();
        assert_eq!(level.level_name, "world");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_app_waits_for_a_departing_player() {
        let mut app = App::new();
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

    #[cfg(unix)]
    #[test]
    fn a_failed_host_file_write_fails_the_exit() {
        use std::os::unix::fs::PermissionsExt;
        let dir =
            std::env::temp_dir().join(format!("mcrs-shutdown-readonly-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o555)).unwrap();
        let probe = dir.join("probe");
        if std::fs::write(&probe, b"").is_ok() {
            let _ = std::fs::remove_file(probe);
            std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o755)).unwrap();
            let _ = std::fs::remove_dir_all(dir);
            return;
        }
        let mut app = App::new();
        app.add_plugins(ShutdownPlugin);
        app.insert_resource(mcrs_minecraft_engine_world::world::world_save::WorldSave(
            dir.clone(),
        ));
        app.world_mut().write_message(ShutdownRequest);
        for _ in 0..200 {
            app.update();
            if app.should_exit().is_some() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        assert_eq!(app.should_exit(), Some(AppExit::error()));
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o755)).unwrap();
        let _ = std::fs::remove_dir_all(dir);
    }
}
