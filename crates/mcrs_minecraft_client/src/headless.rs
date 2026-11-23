use std::path::PathBuf;
use std::time::Duration;

use bevy::app::ScheduleRunnerPlugin;
use bevy::app::TaskPoolPlugin;
use bevy::asset::io::AssetSourceId;
use bevy::log::LogPlugin;
use bevy::prelude::*;
use mcrs_minecraft_assets::packs::PackRoots;
use mcrs_minecraft_block::definition::Blocks;
use mcrs_minecraft_client_entity::ClientEntityPlugin;
use mcrs_minecraft_client_world::asset_source::asset_source;
use mcrs_minecraft_client_world::block_breaking::BlockBreakingPlugin;
use mcrs_minecraft_client_world::chat::ClientChatPlugin;
use mcrs_minecraft_client_world::columns::ColumnCachePlugin;
use mcrs_minecraft_client_world::game_mode::GameModePlugin;
use mcrs_minecraft_client_world::game_packets::GAME_CLIENTBOUND;
use mcrs_minecraft_client_world::hotbar_sync::HotbarSyncPlugin;
use mcrs_minecraft_client_world::local_player;
use mcrs_minecraft_client_world::local_player::LocalPlayerPlugin;
use mcrs_minecraft_client_world::player_view::RequestedViewDistance;
use mcrs_minecraft_client_world::registries::ClientRegistriesPlugin;
use mcrs_minecraft_datapack::DirectoryPacks;
use mcrs_minecraft_network::client::ClientNetworkPlugin;
use mcrs_minecraft_network::client::JoinRequest;

const FRAME: Duration = Duration::from_millis(10);

pub struct HeadlessClientPlugin {
    pub packs: PackRoots,
    pub join: JoinRequest,
}

impl Plugin for HeadlessClientPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            TaskPoolPlugin::default(),
            bevy::time::TimePlugin,
            ScheduleRunnerPlugin::run_loop(FRAME),
            LogPlugin::default(),
        ))
        .register_asset_source(AssetSourceId::Default, asset_source(&self.packs))
        .add_plugins((
            AssetPlugin {
                file_path: self.packs.assets.clone(),
                watch_for_changes_override: Some(false),
                ..default()
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
            ColumnCachePlugin::default(),
            GameModePlugin,
            ClientEntityPlugin,
            ClientChatPlugin {
                command_history: None,
            },
            LocalPlayerPlugin,
            BlockBreakingPlugin,
            HotbarSyncPlugin,
        ))
        .insert_resource(Time::<Fixed>::from_hz(local_player::TICKS_PER_SECOND))
        .insert_resource(RequestedViewDistance(self.join.view_distance));
        join_when_loaded(app, self.join.clone());
    }
}

pub fn join_when_loaded(app: &mut App, join: JoinRequest) {
    app.add_systems(
        Update,
        (move |mut requests: MessageWriter<JoinRequest>| {
            requests.write(join.clone());
        })
        .run_if(resource_added::<Blocks>),
    );
}
