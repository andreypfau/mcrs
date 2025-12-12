extern crate core;

mod biome;
mod client_info;
mod configuration;
pub mod dialog;
mod dimension_type;
mod direction;
mod keep_alive;
mod login;
mod parallel_world;
mod sound;
mod value;
mod version;
mod weight;
pub mod world;

use crate::client_info::ClientInfoPlugin;
use crate::configuration::ConfigurationStatePlugin;
use crate::keep_alive::KeepAlivePlugin;
use crate::login::LoginPlugin;
use crate::world::WorldPlugin;
use bevy_app::{App, Plugin};
use bevy_time::{Fixed, Time, TimePlugin};
use mcrs_network::{EngineConnection, NetworkPlugin};
use std::num::NonZeroU32;

pub const DEFAULT_TPS: NonZeroU32 = match NonZeroU32::new(20) {
    Some(n) => n,
    None => unreachable!(),
};

pub struct ServerPlugin;

impl Plugin for ServerPlugin {
    fn build(&self, app: &mut App) {
        if !app.is_plugin_added::<TimePlugin>() {
            app.add_plugins(TimePlugin);
        }
        app.insert_resource(Time::<Fixed>::from_hz(DEFAULT_TPS.get() as f64));
        // if !app.is_plugin_added::<ScheduleRunnerPlugin>() {
        //     app.add_plugins(ScheduleRunnerPlugin::default());
        // }
        app.add_plugins(NetworkPlugin);
        app.add_plugins(LoginPlugin);
        app.add_plugins(ConfigurationStatePlugin);
        app.add_plugins(KeepAlivePlugin);
        app.add_plugins(WorldPlugin);
        app.add_plugins(ClientInfoPlugin);
    }
}
