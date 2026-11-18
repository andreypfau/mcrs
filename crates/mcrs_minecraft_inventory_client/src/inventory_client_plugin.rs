use bevy_app::App;
use bevy_app::FixedUpdate;
use bevy_app::Plugin;
use bevy_app::Update;
use bevy_ecs::schedule::IntoScheduleConfigs;
use bevy_ecs::schedule::common_conditions::on_message;
use mcrs_minecraft_engine_inventory_click::MenuClickPlugin;
use mcrs_minecraft_engine_item::ItemAppExt;
use mcrs_minecraft_network::client::ClientNetworkSystems;
use mcrs_minecraft_player_holders::PlayerHolders;

use crate::HandInput;
use crate::InventoryClientSet;
use crate::MenuInput;
use crate::apply_menu_inputs::apply_menu_inputs;
use crate::click_source::ClickSource;
use crate::container_close::container_close;
use crate::container_set_content::container_set_content;
use crate::container_set_slot::container_set_slot;
use crate::creative_drop_counter::CreativeDropCounter;
use crate::creative_drop_counter::count_down_creative_drops;
use crate::creative_drop_counter::creative_drops_pending;
use crate::empty_creative_cursor::empty_creative_cursor;
use crate::open_screen::open_screen;
use crate::outbox::Outbox;
use crate::outbox::outbox_has_packets;
use crate::predict_click::ClickScratch;
use crate::predict_hands::predict_hands;
use crate::respawn::respawn;
use crate::send_outbox::send_outbox;
use crate::set_cursor_item::set_cursor_item;
use crate::set_player_inventory::set_player_inventory;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct InventoryClientPlugin;

impl Plugin for InventoryClientPlugin {
    fn build(&self, app: &mut App) {
        if !app.is_plugin_added::<MenuClickPlugin>() {
            app.add_plugins(MenuClickPlugin);
        }
        match app.allocate_request_source() {
            Ok(source) => {
                app.insert_resource(ClickSource(source));
            }
            Err(error) => tracing::warn!(%error, "the client predicts no click"),
        }
        if let Err(error) =
            app.try_register_required_components::<PlayerHolders, CreativeDropCounter>()
        {
            tracing::warn!(%error, "the client counts no creative drop");
        }
        app.add_message::<MenuInput>()
            .add_message::<HandInput>()
            .init_resource::<ClickScratch>()
            .init_resource::<Outbox>()
            .configure_sets(
                FixedUpdate,
                InventoryClientSet::Predict.before(InventoryClientSet::Send),
            )
            .configure_sets(
                Update,
                (
                    ClientNetworkSystems::Receive,
                    InventoryClientSet::Predict,
                    InventoryClientSet::Send,
                    ClientNetworkSystems::Flush,
                )
                    .chain(),
            )
            .add_systems(
                FixedUpdate,
                (
                    predict_hands
                        .run_if(on_message::<HandInput>)
                        .in_set(InventoryClientSet::Predict),
                    count_down_creative_drops.run_if(creative_drops_pending),
                    send_outbox
                        .in_set(InventoryClientSet::Send)
                        .run_if(outbox_has_packets),
                ),
            )
            .add_systems(
                Update,
                (
                    apply_menu_inputs
                        .run_if(on_message::<MenuInput>)
                        .in_set(InventoryClientSet::Predict),
                    send_outbox
                        .in_set(InventoryClientSet::Send)
                        .run_if(outbox_has_packets),
                ),
            )
            .add_observer(open_screen)
            .add_observer(container_set_content)
            .add_observer(container_set_slot)
            .add_observer(set_cursor_item)
            .add_observer(set_player_inventory)
            .add_observer(respawn)
            .add_observer(container_close)
            .add_observer(empty_creative_cursor);
    }
}
