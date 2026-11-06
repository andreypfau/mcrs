#[cfg(feature = "locate")]
use crate::locate;
use crate::{execute, gamemode, teleport, time, whitelist};
use bevy_app::{App, Plugin};
use core::fmt::Display;
use mcrs_minecraft_command_engine::{AppCommandsExt, Worlds};

/// The vanilla commands: `execute`, `gamemode`, `teleport` with its alias `tp`, `locate` (feature
/// `locate`) and `time`, and their effects: [`TeleportPlayer`](crate::TeleportPlayer),
/// [`SetGameMode`](crate::SetGameMode) and
/// [`EditClock`](mcrs_minecraft_engine::coordinator::clock::EditClock).
pub struct VanillaCommandsPlugin;

impl Plugin for VanillaCommandsPlugin {
    fn build(&self, app: &mut App) {
        app.add_command_effect(teleport::apply)
            .add_command_effect(gamemode::apply)
            .add_command_effect(|worlds: &mut Worlds<'_>, edit| worlds.edit_clock(edit))
            .add_command_effect(whitelist::apply);
        report(
            "execute",
            app.add_modifying_command(execute::build, execute::run, execute::modify),
        );
        report("gamemode", app.add_command(gamemode::build, gamemode::run));
        report("teleport", app.add_command(teleport::build, teleport::run));
        #[cfg(feature = "locate")]
        report("locate", app.add_command(locate::build, locate::run));
        report("time", app.add_command(time::build, time::run));
        report(
            "whitelist",
            app.add_command(whitelist::build, whitelist::run),
        );
    }
}

fn report<T, E: Display>(name: &str, result: Result<T, E>) {
    if let Err(error) = result {
        tracing::error!("{name}: {error}");
    }
}
