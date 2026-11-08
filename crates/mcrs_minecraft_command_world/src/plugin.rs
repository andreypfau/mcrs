use bevy_app::{App, Plugin, Update};
use bevy_ecs::prelude::*;
use mcrs_minecraft_command_engine::{AppCommandsExt, RegistrationError, system_message};
use mcrs_minecraft_engine::bus::OutboundPlayerPacket;
use mcrs_minecraft_engine::muxer::online_count::send_online_count;
use mcrs_minecraft_engine_world::session::PlayerSession;
use mcrs_minecraft_engine_world::tick_pace::TickPace;
use mcrs_minecraft_engine_world::world::sub_app::AddDimensionPlugins;

use crate::forced_columns::ForcedColumns;
use crate::{fill, forceload, gamerule, server_change, tick, weather};

/// The commands `fill`, `forceload`, `gamerule`, `weather` and `tick` of the vanilla command
/// set, the effect [`ServerChange`](crate::ServerChange) they write, and the report of a sprint
/// in the server log.
pub struct WorldCommandsPlugin;

impl Plugin for WorldCommandsPlugin {
    fn build(&self, app: &mut App) {
        app.add_command_effect(server_change::apply);
        report("fill", app.add_command(fill::build, fill::run));
        report(
            "forceload",
            app.add_command(forceload::build, forceload::run),
        );
        report("gamerule", app.add_command(gamerule::build, gamerule::run));
        report("weather", app.add_command(weather::build, weather::run));
        report("tick", app.add_command(tick::build, tick::run));
        app.add_dimension_plugin(ForcedColumnsPlugin);
        app.add_systems(Update, report_sprint.after(send_online_count));
    }
}

fn report<T>(name: &str, result: Result<T, RegistrationError>) {
    if let Err(error) = result {
        tracing::error!("{name}: {error}");
    }
}

/// The forced columns of a dimension world.
#[derive(Clone)]
struct ForcedColumnsPlugin;

impl Plugin for ForcedColumnsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ForcedColumns>();
    }
}

/// Writes the report of a sprint that ended to the server log, as the server source does.
fn report_sprint(pace: Option<ResMut<TickPace>>, mut packets: MessageWriter<OutboundPlayerPacket>) {
    let Some(mut pace) = pace else {
        return;
    };
    let Some(report) = pace.take_report() else {
        return;
    };
    let message = tick::report_message(report, &pace);
    packets.write_batch(system_message(PlayerSession::SERVER, &message));
}
