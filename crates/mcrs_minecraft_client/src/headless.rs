use std::path::PathBuf;
use std::time::Duration;

use bevy::app::ScheduleRunnerPlugin;
use bevy::app::TaskPoolPlugin;
use bevy::asset::io::AssetSourceId;
use bevy::log::LogPlugin;
use bevy::prelude::*;
use mcrs_minecraft_assets::packs::PackRoots;
use mcrs_minecraft_client_world::asset_source::asset_source;
use mcrs_minecraft_client_world::player_view::RequestedViewDistance;
use mcrs_minecraft_client_world::registries::ClientRegistriesPlugin;
use mcrs_minecraft_datapack::DirectoryPacks;
use mcrs_minecraft_network::client::JoinRequest;

use crate::ClientCorePlugins;

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
            ClientCorePlugins {
                join: Some(self.join.clone()),
            }
            .set(ClientRegistriesPlugin {
                local_corpus: Some(DirectoryPacks::new(
                    PathBuf::from(&self.packs.assets),
                    self.packs.vanilla.clone(),
                )),
                defer_definitions: false,
            }),
        ))
        .insert_resource(RequestedViewDistance(self.join.view_distance));
    }
}
