use bevy_app::App;
use bevy_app::Plugin;
use mcrs_minecraft_engine::persisted_registry::PersistScope;
use mcrs_minecraft_engine::persisted_registry::PersistedAppExt;

use crate::carried_codec::CARRIED_KEYS;
use crate::carried_codec::apply_carried;
use crate::carried_codec::capture_carried;
use crate::player_file::FILE_KEYS;
use crate::player_file_codec::apply_player_file;
use crate::player_file_codec::capture_player_file;

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
