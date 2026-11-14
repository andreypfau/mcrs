use bevy_app::App;
use bevy_app::Plugin;
use mcrs_minecraft_engine::persisted_registry::PersistScope;
use mcrs_minecraft_engine::persisted_registry::PersistedAppExt;

use crate::carried_codec::apply_carried;
use crate::carried_codec::capture_carried;
use crate::player_file::FILE_KEYS;
use crate::player_file_codec::apply_player_file;
use crate::player_file_codec::capture_player_file;
use crate::player_stacks::CARRIED_KEYS;

/// The codecs of the four holders of each player: `Inventory`, `SelectedItemSlot` and `equipment`
/// of the player file (INV-SAV-001, INV-SAV-002), and the cursor and the craft grid that a cell
/// crossing carries. A leave and a dimension change first close the inventory menu (INV-CLS-001,
/// INV-CLS-003).
///
/// It replaces [`PlayerItemFilePlugin`](mcrs_minecraft_engine_item::PlayerItemFilePlugin): an app
/// adds one of the two, never both. Add it with
/// [`PersistedAppExt::add_persistence_plugin`], beside [`VanillaInventoryPlugin`](crate::VanillaInventoryPlugin)
/// in each dimension world.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct InventoryPersistencePlugin;

impl Plugin for InventoryPersistencePlugin {
    fn build(&self, app: &mut App) {
        app.register_persisted_codec(
            FILE_KEYS,
            PersistScope::Persisted,
            capture_player_file,
            apply_player_file,
        )
        .register_persisted_codec(
            CARRIED_KEYS,
            PersistScope::TravelOnly,
            capture_carried,
            apply_carried,
        );
    }
}
