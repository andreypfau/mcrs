mod entity_motion;
mod falling_blocks;
mod item_entities;
pub mod operator_broadcast;
mod player_pose;
mod vanilla_blocks;
mod vanilla_brains;
mod vanilla_death_drops;
mod vanilla_death_messages;
mod vanilla_swing;

use bevy_app::{App, Plugin, PluginGroup, PluginGroupBuilder};
use mcrs_minecraft_chat_server::ChatPlugin;
use mcrs_minecraft_command_engine::CommandPlugin;
use mcrs_minecraft_command_vanilla::{VanillaCommands, VanillaCommandsPlugin};
use mcrs_minecraft_command_world::WorldCommandsPlugin;
use mcrs_minecraft_datapack::REPOSITORY_PACKS;
use mcrs_minecraft_engine::persisted_registry::PersistedAppExt;
use mcrs_minecraft_engine_attributes::PlayerAttributesPlugin;
use mcrs_minecraft_engine_brain::MobBrainPlugin;
use mcrs_minecraft_engine_item::ItemPersistencePlugin;
use mcrs_minecraft_engine_mob_attributes::MobAttributesPlugin;
use mcrs_minecraft_engine_player_vitals::PlayerVitalsPlugin;
use mcrs_minecraft_engine_server::{DimensionWorldsPlugin, ServerCorePlugin, ServerNetworkPlugin};
use mcrs_minecraft_engine_world::world::dimension_builders::DimensionBuilders;
use mcrs_minecraft_engine_world::world::sub_app::AddDimensionPlugins;
use mcrs_minecraft_entity::damage_type_tag_keys::PANIC_CAUSES;
use mcrs_minecraft_vanilla_attributes::{CreativeRangePlugin, SprintAttributePlugin};
use mcrs_minecraft_vanilla_block_place::BlockPlacePlugin;
use mcrs_minecraft_vanilla_inventory::InventoryPersistencePlugin;
use mcrs_minecraft_vanilla_light::LightingPlugin;
use mcrs_minecraft_vanilla_player_attack::PlayerAttackPlugin;
use mcrs_minecraft_vanilla_respawn::RespawnPlugin;

pub use falling_blocks::VanillaFallingBlockPlugin;
pub use item_entities::VanillaItemEntityPlugin;
use mcrs_minecraft_vanilla_worldgen::{WorldGenPlugin, WorldgenRegistriesPlugin};
pub use vanilla_blocks::{VanillaBlocks, VanillaBlocksPlugin};
pub use vanilla_death_drops::VanillaDeathDropsPlugin;
pub use vanilla_death_messages::VanillaDeathMessagesPlugin;
pub use vanilla_swing::VanillaSwingPlugin;

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
                local_packs: REPOSITORY_PACKS.map(str::to_owned).to_vec(),
                ..Default::default()
            },
            worlds: DimensionWorldsPlugin::default(),
            world_gen: WorldGenPlugin,
            network: ServerNetworkPlugin::default(),
        }
    }
}

impl MinecraftServerPlugins {
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
            .add(WorldCommandsPlugin)
            .add(self.worlds)
            .add(VanillaChatPlugin)
            .add(VanillaEntityPhysicsPlugin)
            .add(VanillaPlayerPosePlugin)
            .add(VanillaEntityAttributesPlugin)
            .add(VanillaRespawnPlugin)
            .add(VanillaDeathDropsPlugin)
            .add(VanillaDeathMessagesPlugin)
            .add(VanillaSprintAttributePlugin)
            .add(VanillaCreativeRangePlugin)
            .add(VanillaItemEntityPlugin)
            .add(VanillaFallingBlockPlugin)
            .add(VanillaBlocksPlugin)
            .add(VanillaMobBrainPlugin)
            .add(VanillaItemPlugin)
            .add(VanillaBlockPlacePlugin)
            .add(VanillaSwingPlugin)
            .add(VanillaPlayerAttackPlugin)
            .add(self.world_gen)
            .add(LightingPlugin)
            .add(self.network)
    }
}

pub struct VanillaEntityPhysicsPlugin;

impl Plugin for VanillaEntityPhysicsPlugin {
    fn build(&self, app: &mut App) {
        app.world_mut()
            .get_resource_or_init::<DimensionBuilders>()
            .add(entity_motion::build_dimension_mobs);
    }
}

pub struct VanillaPlayerPosePlugin;

impl Plugin for VanillaPlayerPosePlugin {
    fn build(&self, app: &mut App) {
        app.add_dimension_plugin(player_pose::PlayerPosePlugin);
    }
}

pub struct VanillaEntityAttributesPlugin;

impl Plugin for VanillaEntityAttributesPlugin {
    fn build(&self, app: &mut App) {
        app.add_dimension_plugin(PlayerAttributesPlugin);
        app.add_dimension_plugin(PlayerVitalsPlugin);
        app.add_dimension_plugin(MobAttributesPlugin);
    }
}

pub struct VanillaRespawnPlugin;

impl Plugin for VanillaRespawnPlugin {
    fn build(&self, app: &mut App) {
        app.add_dimension_plugin(RespawnPlugin);
    }
}

pub struct VanillaSprintAttributePlugin;

impl Plugin for VanillaSprintAttributePlugin {
    fn build(&self, app: &mut App) {
        app.add_dimension_plugin(SprintAttributePlugin);
    }
}

pub struct VanillaCreativeRangePlugin;

impl Plugin for VanillaCreativeRangePlugin {
    fn build(&self, app: &mut App) {
        app.add_dimension_plugin(CreativeRangePlugin);
    }
}

pub struct VanillaMobBrainPlugin;

impl Plugin for VanillaMobBrainPlugin {
    fn build(&self, app: &mut App) {
        app.add_dimension_plugin(MobBrainPlugin::new(
            vanilla_brains::vanilla_brain,
            PANIC_CAUSES,
        ));
    }
}

pub struct VanillaItemPlugin;

impl Plugin for VanillaItemPlugin {
    fn build(&self, app: &mut App) {
        app.add_persistence_plugin(ItemPersistencePlugin)
            .add_persistence_plugin(InventoryPersistencePlugin);
    }
}

pub struct VanillaBlockPlacePlugin;

impl Plugin for VanillaBlockPlacePlugin {
    fn build(&self, app: &mut App) {
        app.add_dimension_plugin(BlockPlacePlugin);
    }
}

pub struct VanillaPlayerAttackPlugin;

impl Plugin for VanillaPlayerAttackPlugin {
    fn build(&self, app: &mut App) {
        app.add_dimension_plugin(PlayerAttackPlugin);
    }
}

pub struct VanillaChatPlugin;

impl Plugin for VanillaChatPlugin {
    fn build(&self, app: &mut App) {
        app.add_dimension_plugin(ChatPlugin);
        app.add_dimension_plugin(operator_broadcast::OperatorBroadcastPlugin);
    }
}
