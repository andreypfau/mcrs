use bevy_app::App;
use bevy_app::FixedUpdate;
use bevy_app::Plugin;
use bevy_ecs::schedule::IntoScheduleConfigs;
use bevy_ecs::schedule::common_conditions::on_message;
use mcrs_minecraft_engine_inventory_menu::MenuPlugin;
use mcrs_minecraft_engine_item::RequestDrop;
use mcrs_minecraft_engine_world::tick::TickSet;

use crate::InventorySet;
use crate::close_menu_moves::close_menu_moves;
use crate::launch_drops::launch_drops;
use crate::restore_holders::hold_new_player;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct VanillaInventoryPlugin;

impl Plugin for VanillaInventoryPlugin {
    fn build(&self, app: &mut App) {
        if !app.is_plugin_added::<MenuPlugin>() {
            app.add_plugins(MenuPlugin);
        }
        app.configure_sets(
            FixedUpdate,
            (
                InventorySet::Input.in_set(TickSet::ClientInput),
                InventorySet::Drops
                    .after(TickSet::Project)
                    .before(TickSet::Output),
            )
                .chain(),
        )
        .add_systems(
            FixedUpdate,
            launch_drops
                .run_if(on_message::<RequestDrop>)
                .in_set(InventorySet::Drops),
        )
        .add_observer(hold_new_player)
        .add_observer(close_menu_moves);
    }
}
