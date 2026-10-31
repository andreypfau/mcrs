pub mod operator_broadcast;

use bevy_app::{App, Plugin, PluginGroup, PluginGroupBuilder};
use mcrs_minecraft_chat_server::ChatPlugin;
use mcrs_minecraft_command_engine::CommandPlugin;
use mcrs_minecraft_command_vanilla::{VanillaCommands, VanillaCommandsPlugin};
use mcrs_minecraft_engine_server::{DimensionWorldsPlugin, ServerCorePlugin, ServerNetworkPlugin};
use mcrs_minecraft_level::world::sub_app::AddDimensionPlugins;
use mcrs_minecraft_vanilla_light::LightingPlugin;
use mcrs_minecraft_vanilla_worldgen::{WorldGenPlugin, WorldgenRegistriesPlugin};

/// The vanilla server: the engine's server plugins with every vanilla member, each one a
/// member that can be disabled or replaced through [`PluginGroup::build`]. A member that needs
/// another one panics at build and names it.
pub struct MinecraftServerPlugins {
    pub core: ServerCorePlugin,
    pub worlds: DimensionWorldsPlugin,
    pub world_gen: WorldGenPlugin,
    pub network: ServerNetworkPlugin,
}

impl Default for MinecraftServerPlugins {
    fn default() -> Self {
        Self {
            core: ServerCorePlugin {
                builtin_assets: mcrs_minecraft_worldgen_builtin::asset,
                ..Default::default()
            },
            worlds: DimensionWorldsPlugin::default(),
            world_gen: WorldGenPlugin::default(),
            network: ServerNetworkPlugin::default(),
        }
    }
}

impl MinecraftServerPlugins {
    /// A server running inside another process: loopback only, an OS-assigned
    /// port, and the task pools left to the host app.
    pub fn embedded() -> Self {
        let defaults = Self::default();
        Self {
            core: ServerCorePlugin {
                owns_task_pools: false,
                ..defaults.core
            },
            network: ServerNetworkPlugin::embedded(),
            ..defaults
        }
    }
}

impl PluginGroup for MinecraftServerPlugins {
    fn build(self) -> PluginGroupBuilder {
        PluginGroupBuilder::start::<Self>()
            .add(self.core)
            .add(WorldgenRegistriesPlugin)
            .add(CommandPlugin::<VanillaCommands>::new())
            .add(VanillaCommandsPlugin)
            .add(self.worlds)
            .add(VanillaChatPlugin)
            .add(self.world_gen)
            .add(LightingPlugin)
            .add(self.network)
    }
}

/// Chat in every dimension world, and the operator messages of commands delivered to the
/// operators.
pub struct VanillaChatPlugin;

impl Plugin for VanillaChatPlugin {
    fn build(&self, app: &mut App) {
        app.add_dimension_plugin(ChatPlugin);
        app.add_observer(operator_broadcast::broadcast_to_operators);
    }
}
