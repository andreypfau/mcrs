use bevy_app::App;
use bevy_app::Plugin;

use crate::container_set_content::container_set_content;
use crate::container_set_slot::container_set_slot;
use crate::respawn::respawn;
use crate::set_cursor_item::set_cursor_item;
use crate::set_player_inventory::set_player_inventory;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct InventoryClientPlugin;

impl Plugin for InventoryClientPlugin {
    fn build(&self, app: &mut App) {
        app.add_observer(container_set_content)
            .add_observer(container_set_slot)
            .add_observer(set_cursor_item)
            .add_observer(set_player_inventory)
            .add_observer(respawn);
    }
}
