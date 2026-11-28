use bevy::app::PluginGroupBuilder;
use bevy::prelude::*;
use mcrs_minecraft_block::definition::Blocks;
use mcrs_minecraft_client_entity::ClientEntityPlugin;
use mcrs_minecraft_client_world::GuiTickPlugin;
use mcrs_minecraft_client_world::block_breaking::BlockBreakingPlugin;
use mcrs_minecraft_client_world::chat::ClientChatPlugin;
use mcrs_minecraft_client_world::columns::ColumnCachePlugin;
use mcrs_minecraft_client_world::game_mode::GameModePlugin;
use mcrs_minecraft_client_world::game_packets::GAME_CLIENTBOUND;
use mcrs_minecraft_client_world::hotbar_sync::HotbarSyncPlugin;
use mcrs_minecraft_client_world::local_player;
use mcrs_minecraft_client_world::local_player::LocalPlayerPlugin;
use mcrs_minecraft_client_world::registries::ClientRegistriesPlugin;
use mcrs_minecraft_network::client::ClientNetworkPlugin;
use mcrs_minecraft_network::client::JoinRequest;

pub struct ClientCorePlugins {
    pub join: Option<JoinRequest>,
}

impl PluginGroup for ClientCorePlugins {
    fn build(self) -> PluginGroupBuilder {
        PluginGroupBuilder::start::<Self>()
            .add(ClientNetworkPlugin {
                holds_vanilla_pack: true,
                play: Some(GAME_CLIENTBOUND),
            })
            .add(ClientRegistriesPlugin::default())
            .add(ColumnCachePlugin::default())
            .add(GameModePlugin)
            .add(ClientEntityPlugin)
            .add(GuiTickPlugin)
            .add(ClientChatPlugin {
                command_history: None,
            })
            .add(LocalPlayerPlugin)
            .add(BlockBreakingPlugin)
            .add(HotbarSyncPlugin)
            .add(ClientTickPlugin { join: self.join })
    }
}

struct ClientTickPlugin {
    join: Option<JoinRequest>,
}

impl Plugin for ClientTickPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(Time::<Fixed>::from_hz(local_player::TICKS_PER_SECOND));
        if let Some(join) = self.join.clone() {
            app.add_systems(
                Update,
                (move |mut requests: MessageWriter<JoinRequest>| {
                    requests.write(join.clone());
                })
                .run_if(resource_added::<Blocks>),
            );
        }
    }
}
