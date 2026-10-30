use crate::{execute, gamemode, locate, teleport, time};
use bevy_app::{App, Plugin};
use mcrs_minecraft_command_engine::AppCommandsExt;

/// The vanilla commands: `execute`, `gamemode`, `teleport` with its alias `tp`, `locate` and
/// `time`, and their effects: [`TeleportPlayer`](crate::TeleportPlayer),
/// [`SetGameMode`](crate::SetGameMode) and
/// [`EditClock`](mcrs_minecraft_engine::clock::EditClock).
pub struct VanillaCommandsPlugin;

impl Plugin for VanillaCommandsPlugin {
    fn build(&self, app: &mut App) {
        app.add_command_effect(teleport::apply)
            .add_command_effect(gamemode::apply)
            .add_command_effect(time::apply);
        let added = [
            (
                "execute",
                app.add_modifying_command(execute::build, execute::run, execute::modify)
                    .map(drop),
            ),
            (
                "gamemode",
                app.add_command(gamemode::build, gamemode::run).map(drop),
            ),
            (
                "teleport",
                app.add_command(teleport::build, teleport::run).map(drop),
            ),
            (
                "locate",
                app.add_command(locate::build, locate::run).map(drop),
            ),
            ("time", app.add_command(time::build, time::run).map(drop)),
        ];
        for (name, result) in added {
            if let Err(error) = result {
                tracing::error!("{name}: {error}");
            }
        }
    }
}
