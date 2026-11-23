#[cfg(feature = "locate")]
use crate::locate;
use crate::{
    attribute, ban, damage, data, execute, gamemode, kill, list, mob_hit, op, summon, teleport,
    time, whitelist,
};
use bevy_app::{App, Plugin};
use bevy_ecs::system::IntoSystem;
use core::fmt::Display;
use mcrs_minecraft_command_engine::{AppCommandsExt, Worlds};
use mcrs_minecraft_engine_world::world::dimension_builders::DimensionBuilders;

pub struct VanillaCommandsPlugin;

impl Plugin for VanillaCommandsPlugin {
    fn build(&self, app: &mut App) {
        app.add_command_effect(teleport::apply)
            .add_command_effect(gamemode::apply)
            .add_command_effect(|worlds: &mut Worlds<'_>, edit| worlds.edit_clock(edit))
            .add_command_effect(whitelist::apply)
            .add_command_effect(ban::apply)
            .add_command_effect(op::apply)
            .add_command_effect(summon::apply)
            .add_command_effect(summon::apply_item)
            .add_command_effect(mob_hit::apply)
            .add_command_effect(attribute::apply);
        report(
            "execute",
            app.add_modifying_command(execute::build, execute::run, execute::modify),
        );
        report("gamemode", app.add_command(gamemode::build, gamemode::run));
        report("teleport", app.add_command(teleport::build, teleport::run));
        #[cfg(feature = "locate")]
        report("locate", app.add_command(locate::build, locate::run));
        report("summon", app.add_command(summon::build, summon::run));
        report("kill", app.add_command(kill::build, kill::run));
        report("damage", app.add_command(damage::build, damage::run));
        report(
            "attribute",
            app.add_command(attribute::build, attribute::run),
        );
        report(
            "data",
            app.add_command(data::build, data::find.pipe(data::read).pipe(data::reply)),
        );
        report("time", app.add_command(time::build, time::run));
        report("ban", app.add_command(ban::build, ban::run));
        report("op", app.add_command(op::build, op::run));
        report("list", app.add_command(list::build, list::run));
        app.world_mut()
            .get_resource_or_init::<DimensionBuilders>()
            .add(list::copy_join_limits);
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
