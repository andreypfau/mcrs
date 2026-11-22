use bevy_app::App;
use bevy_app::FixedUpdate;
use bevy_app::Plugin;
use bevy_app::Update;
use bevy_ecs::schedule::IntoScheduleConfigs;
use bevy_ecs::schedule::common_conditions::on_message;
use mcrs_minecraft_engine::bus::InboundPlayerDespawn;
use mcrs_minecraft_engine::despawn_inbound_player::despawn_inbound_player;
use mcrs_minecraft_engine_damage::DamageSet;
use mcrs_minecraft_engine_damage::Died;
use mcrs_minecraft_engine_damage::RespawnRequest;
use mcrs_minecraft_player_holders::EnchantmentEffectsPlugin;

use crate::drop_held_stacks::drop_held_at_departure;
use crate::drop_held_stacks::drop_held_at_removal;
use crate::drop_held_stacks::drop_held_at_respawn;
use crate::drop_inventory::drop_inventory;

#[derive(Clone, Copy, Debug, Default)]
pub struct DeathDropsPlugin;

impl Plugin for DeathDropsPlugin {
    fn build(&self, app: &mut App) {
        if !app.is_plugin_added::<EnchantmentEffectsPlugin>() {
            app.add_plugins(EnchantmentEffectsPlugin);
        }
        app.add_observer(drop_held_at_removal)
            .add_message::<Died>()
            .add_message::<RespawnRequest>()
            .add_message::<InboundPlayerDespawn>()
            .add_systems(
                FixedUpdate,
                (
                    drop_inventory.run_if(on_message::<Died>),
                    drop_held_at_respawn.run_if(on_message::<RespawnRequest>),
                )
                    .in_set(DamageSet::Report),
            )
            .add_systems(
                Update,
                drop_held_at_departure
                    .run_if(on_message::<InboundPlayerDespawn>)
                    .before(despawn_inbound_player),
            );
    }
}
