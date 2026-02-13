#![recursion_limit = "2048"]
#![allow(
    dead_code,
    unused_variables,
    unused_imports,
    unused_mut,
    unused_parens,
    unreachable_pub,
    unexpected_cfgs,
    non_camel_case_types,
    private_interfaces,
    clippy::uninlined_format_args,
    clippy::type_complexity,
    clippy::needless_borrow,
    clippy::expect_fun_call,
    clippy::useless_vec,
    clippy::assign_op_pattern,
    clippy::collapsible_if,
    clippy::option_map_unit_fn,
    clippy::map_flatten,
    clippy::too_many_arguments,
    clippy::empty_line_after_doc_comments,
    clippy::derivable_impls,
    clippy::useless_conversion,
    clippy::no_effect,
    clippy::from_over_into,
    clippy::needless_update,
    clippy::unnecessary_fallible_conversions
)]

extern crate core;

mod biome;
mod client_info;
mod configuration;
pub mod dialog;
mod dimension_type;
mod direction;
mod keep_alive;
mod login;
pub mod sound;
mod tag;
mod value;
mod version;
mod weight;
pub mod world;
pub mod world_preset_loader;

use crate::client_info::ClientInfoPlugin;
use crate::configuration::ConfigurationStatePlugin;
use crate::keep_alive::KeepAlivePlugin;
use crate::login::LoginPlugin;
use crate::tag::{BlockTagPlugin, ItemTagPlugin};
use crate::world::WorldPlugin;
use bevy_app::{App, Plugin};
use bevy_asset::AssetPlugin;
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
        app.add_plugins(AssetPlugin::default());
        app.add_plugins(BlockTagPlugin);
        app.add_plugins(ItemTagPlugin);
        app.add_plugins(NetworkPlugin);
        app.add_plugins(LoginPlugin);
        app.add_plugins(ConfigurationStatePlugin);
        app.add_plugins(KeepAlivePlugin);
        app.add_plugins(WorldPlugin);
        app.add_plugins(ClientInfoPlugin);
    }
}
