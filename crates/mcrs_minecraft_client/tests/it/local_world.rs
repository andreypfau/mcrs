use std::path::PathBuf;

use bevy::prelude::*;
use mcrs_minecraft_client::local_server::LocalServerPlugin;
use mcrs_minecraft_client_entity::ClientEntityPlugin;
use mcrs_minecraft_client_world::chat::ClientChatPlugin;
use mcrs_minecraft_client_world::columns::ColumnCachePlugin;
use mcrs_minecraft_client_world::game_mode::GameModePlugin;
use mcrs_minecraft_client_world::game_packets::GAME_CLIENTBOUND;
use mcrs_minecraft_client_world::player_view::RequestedViewDistance;
use mcrs_minecraft_network::client::ClientNetworkPlugin;
use mcrs_minecraft_server_settings::ServerSettings;

use crate::boot::boot;

pub(crate) fn client() -> App {
    boot(|app| {
        app.add_plugins((
            ClientNetworkPlugin {
                holds_vanilla_pack: true,
                play: Some(GAME_CLIENTBOUND),
            },
            ColumnCachePlugin::default(),
            GameModePlugin,
            ClientEntityPlugin,
            ClientChatPlugin {
                command_history: None,
            },
            LocalServerPlugin {
                asset_path: concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets").to_owned(),
                data_pack: mcrs_minecraft_fixtures::vanilla_dir().to_owned(),
                ops_file: None,
                username: "Player".to_owned(),
                open_to_lan: false,
                settings: ServerSettings::default(),
                column_traces: None,
            },
        ))
        .init_resource::<Time<Real>>()
        .insert_resource(RequestedViewDistance(2));
    })
}

pub(crate) fn world_folder(name: &str) -> PathBuf {
    let folder = std::env::temp_dir().join(format!("mcrs-{name}-{}", std::process::id()));
    std::fs::remove_dir_all(&folder).ok();
    folder
}
