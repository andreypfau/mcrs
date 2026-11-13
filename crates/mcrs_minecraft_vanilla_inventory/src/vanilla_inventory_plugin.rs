use bevy_app::App;
use bevy_app::Plugin;

use crate::restore_holders::hold_new_player;

/// Gives each new player its four holders (INV-SLT-002).
///
/// Add it to each dimension world, beside
/// [`ItemPlugin`](mcrs_minecraft_engine_item::ItemPlugin), which it needs. The player file and a
/// handover reach the holders through [`InventoryPersistencePlugin`](crate::InventoryPersistencePlugin).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct VanillaInventoryPlugin;

impl Plugin for VanillaInventoryPlugin {
    fn build(&self, app: &mut App) {
        app.add_observer(hold_new_player);
    }
}
