use std::path::PathBuf;

use bevy_app::App;
use bevy_app::Plugin;
use bevy_app::TaskPoolPlugin;
use bevy_asset::AssetApp;
use bevy_asset::AssetPlugin;
use bevy_asset::io::AssetSourceId;
use bevy_time::TimePlugin;
use mcrs_minecraft_assets::packs::PackRoots;
use mcrs_minecraft_client_world::asset_source::asset_source;
use mcrs_minecraft_client_world::game_mode::GameModePlugin;
use mcrs_minecraft_client_world::game_packets::GAME_CLIENTBOUND;
use mcrs_minecraft_client_world::registries::ClientRegistriesPlugin;
use mcrs_minecraft_datapack::DirectoryPacks;
use mcrs_minecraft_network::client::ClientNetworkPlugin;

/// The client without a window: the connection, the session registries, the game mode
/// and the abilities. `ClientRegistriesPlugin` adds the holders and the inventory edge of the local
/// player.
pub struct HeadlessClientPlugin {
    pub packs: PackRoots,
}

impl Plugin for HeadlessClientPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((TaskPoolPlugin::default(), TimePlugin))
            .register_asset_source(AssetSourceId::Default, asset_source(&self.packs))
            .add_plugins((
                AssetPlugin {
                    file_path: self.packs.assets.clone(),
                    watch_for_changes_override: Some(false),
                    ..AssetPlugin::default()
                },
                ClientNetworkPlugin {
                    holds_vanilla_pack: true,
                    play: Some(GAME_CLIENTBOUND),
                },
                ClientRegistriesPlugin {
                    local_corpus: Some(DirectoryPacks::new(
                        PathBuf::from(&self.packs.assets),
                        self.packs.vanilla.clone(),
                    )),
                    defer_definitions: false,
                },
                GameModePlugin,
            ));
    }
}
