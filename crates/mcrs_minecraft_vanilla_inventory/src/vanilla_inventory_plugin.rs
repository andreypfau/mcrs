use bevy_app::App;
use bevy_app::FixedUpdate;
use bevy_app::Plugin;
use bevy_ecs::schedule::IntoScheduleConfigs;
use bevy_ecs::schedule::common_conditions::on_message;
use mcrs_minecraft_engine::swing::{SwingPlayer, SwingSimulateSet};
use mcrs_minecraft_engine_item::ItemAppExt;
use mcrs_minecraft_engine_item::RequestDrop;
use mcrs_minecraft_engine_world::tick::TickSet;
use mcrs_minecraft_player_holders::PlayerHoldersPlugin;

use crate::InventorySet;
use crate::VanillaInventoryRequests;
use crate::close_menu_moves::close_menu_moves;
use crate::drop_swing::swing_on_hand_drop;
use crate::launch_drops::launch_drops;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct VanillaInventoryPlugin;

impl Plugin for VanillaInventoryPlugin {
    fn build(&self, app: &mut App) {
        if !app.is_plugin_added::<PlayerHoldersPlugin>() {
            app.add_plugins(PlayerHoldersPlugin);
        }
        let source = match app.allocate_request_source() {
            Ok(source) => source,
            Err(error) => {
                tracing::error!(%error, "the vanilla inventory makes no request");
                return;
            }
        };
        app.insert_resource(VanillaInventoryRequests { source });
        app.add_message::<SwingPlayer>();
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
            (
                swing_on_hand_drop
                    .run_if(on_message::<RequestDrop>)
                    .in_set(TickSet::Simulate)
                    .after(SwingSimulateSet),
                launch_drops
                    .run_if(on_message::<RequestDrop>)
                    .in_set(InventorySet::Drops),
            ),
        )
        .add_observer(close_menu_moves);
    }
}
