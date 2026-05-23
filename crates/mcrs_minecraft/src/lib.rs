#![recursion_limit = "2048"]
#![allow(
    dead_code,
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
    clippy::from_over_into,
    clippy::needless_update,
    clippy::unnecessary_fallible_conversions
)]

extern crate core;

mod biome;
mod client_info;
pub mod runner;
pub use runner::run_server_loop;
mod configuration;
pub mod dialog;
mod dimension_type;
mod direction;
pub mod disconnect;
pub mod enchantment;
mod keep_alive;
pub mod login;
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
use crate::world::WorldPlugin;
use bevy_app::prelude::*;
use bevy_app::{App, Plugin, TaskPoolOptions, TaskPoolPlugin};
use bevy_asset::AssetPlugin;
use bevy_ecs::prelude::IntoScheduleConfigs;
use bevy_ecs::schedule::{ExecutorKind, ScheduleLabel};
use bevy_state::prelude::OnEnter;
use bevy_time::{Fixed, Time, TimePlugin};
use mcrs_core::AppState;
use mcrs_minecraft_lighting::table::{build_block_light_table, BlockStateLightTable};
use mcrs_network::NetworkPlugin;
use mcrs_vanilla::{freeze_static_tags, transition_to_playing};
use std::num::NonZeroU32;

pub const DEFAULT_TPS: NonZeroU32 = match NonZeroU32::new(20) {
    Some(n) => n,
    None => unreachable!(),
};

pub struct ServerPlugin;

impl Plugin for ServerPlugin {
    fn build(&self, app: &mut App) {
        #[cfg(debug_assertions)]
        app.add_plugins(TaskPoolPlugin {
            task_pool_options: TaskPoolOptions::with_num_threads(1),
        });
        #[cfg(not(debug_assertions))]
        app.add_plugins(TaskPoolPlugin::default());

        app.edit_schedule(Update, |schedule| {
            schedule.set_executor_kind(ExecutorKind::SingleThreaded);
        });
        #[cfg(debug_assertions)]
        force_singlethread_schedules(app);

        if !app.is_plugin_added::<TimePlugin>() {
            app.add_plugins(TimePlugin);
        }
        app.insert_resource(Time::<Fixed>::from_hz(DEFAULT_TPS.get() as f64));
        app.add_plugins(AssetPlugin::default());
        app.add_plugins(mcrs_core::MinecraftEnginePlugin);
        app.add_plugins(mcrs_vanilla::MinecraftCorePlugin);
        app.add_plugins(NetworkPlugin);
        app.add_plugins(LoginPlugin);
        app.add_plugins(ConfigurationStatePlugin);
        app.add_plugins(KeepAlivePlugin);
        app.add_plugins(WorldPlugin);
        app.init_resource::<BlockStateLightTable>();
        app.add_systems(
            OnEnter(AppState::WorldgenFreeze),
            build_block_light_table
                .after(freeze_static_tags)
                .before(transition_to_playing),
        );
        app.add_plugins(ClientInfoPlugin);
    }
}

#[cfg(debug_assertions)]
fn force_singlethread_schedules(app: &mut App) {
    for label in [
        PreStartup.intern(),
        Startup.intern(),
        PostStartup.intern(),
        First.intern(),
        PreUpdate.intern(),
        RunFixedMainLoop.intern(),
        Update.intern(),
        PostUpdate.intern(),
        Last.intern(),
        FixedFirst.intern(),
        FixedPreUpdate.intern(),
        FixedUpdate.intern(),
        FixedPostUpdate.intern(),
        FixedLast.intern(),
    ] {
        app.edit_schedule(label, |s| {
            s.set_executor_kind(ExecutorKind::SingleThreaded);
        });
    }
}
