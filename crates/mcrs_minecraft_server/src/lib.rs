mod entity_motion;
mod item_entities;
pub mod operator_broadcast;
mod vanilla_blocks;
mod vanilla_brains;

use bevy_app::{App, Plugin, PluginGroup, PluginGroupBuilder};
use mcrs_minecraft_chat_server::ChatPlugin;
use mcrs_minecraft_command_engine::CommandPlugin;
use mcrs_minecraft_command_vanilla::{VanillaCommands, VanillaCommandsPlugin};
use mcrs_minecraft_command_world::WorldCommandsPlugin;
use mcrs_minecraft_engine::persisted_registry::PersistedAppExt;
use mcrs_minecraft_engine_attributes::EntityAttributesPlugin;
use mcrs_minecraft_engine_brain::MobBrainPlugin;
use mcrs_minecraft_engine_brain::MobColumn;
use mcrs_minecraft_engine_item::ItemPersistencePlugin;
use mcrs_minecraft_engine_mob_replication::MobReplicationPlugin;
use mcrs_minecraft_engine_physics::EntityPhysicsPlugin;
use mcrs_minecraft_engine_server::{DimensionWorldsPlugin, ServerCorePlugin, ServerNetworkPlugin};
use mcrs_minecraft_engine_world::world::sub_app::AddDimensionPlugins;
use mcrs_minecraft_entity::damage_type_tag_keys::PANIC_CAUSES;
use mcrs_minecraft_vanilla_attributes::{CreativeRangePlugin, SprintAttributePlugin};
use mcrs_minecraft_vanilla_block_place::BlockPlacePlugin;
use mcrs_minecraft_vanilla_inventory::InventoryPersistencePlugin;
use mcrs_minecraft_vanilla_light::LightingPlugin;

pub use item_entities::VanillaItemEntityPlugin;
use mcrs_minecraft_vanilla_worldgen::{WorldGenPlugin, WorldgenRegistriesPlugin};
pub use vanilla_blocks::{VanillaBlocks, VanillaBlocksPlugin};

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
                // The beta presets name the noise settings the world generation registries build.
                local_packs: vec!["mcrs".to_owned(), "beta".to_owned()],
                ..Default::default()
            },
            worlds: DimensionWorldsPlugin::default(),
            world_gen: WorldGenPlugin,
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
            .add(WorldCommandsPlugin)
            .add(self.worlds)
            .add(VanillaChatPlugin)
            .add(VanillaEntityPhysicsPlugin)
            .add(VanillaEntityAttributesPlugin)
            .add(VanillaSprintAttributePlugin)
            .add(VanillaCreativeRangePlugin)
            .add(VanillaItemEntityPlugin)
            .add(VanillaBlocksPlugin)
            .add(VanillaMobBrainPlugin)
            .add(VanillaItemPlugin)
            .add(VanillaBlockPlacePlugin)
            .add(self.world_gen)
            .add(LightingPlugin)
            .add(self.network)
    }
}

/// The entity pass and the tile events of its mobs in every dimension world.
pub struct VanillaEntityPhysicsPlugin;

impl Plugin for VanillaEntityPhysicsPlugin {
    fn build(&self, app: &mut App) {
        app.add_dimension_plugin(EntityPhysicsPlugin::<MobColumn>::default());
        app.add_dimension_plugin(MobReplicationPlugin::<MobColumn>::default());
    }
}

/// The attribute instances of the players and the mob rows in every dimension world.
pub struct VanillaEntityAttributesPlugin;

impl Plugin for VanillaEntityAttributesPlugin {
    fn build(&self, app: &mut App) {
        app.add_dimension_plugin(EntityAttributesPlugin::<MobColumn>::default());
    }
}

/// The modifier of the sprint state on `movement_speed` in every dimension world. It edits the
/// instances of [`VanillaEntityAttributesPlugin`].
pub struct VanillaSprintAttributePlugin;

impl Plugin for VanillaSprintAttributePlugin {
    fn build(&self, app: &mut App) {
        app.add_dimension_plugin(SprintAttributePlugin);
    }
}

/// The range modifiers of creative mode in every dimension world. It edits the instances of
/// [`VanillaEntityAttributesPlugin`].
pub struct VanillaCreativeRangePlugin;

impl Plugin for VanillaCreativeRangePlugin {
    fn build(&self, app: &mut App) {
        app.add_dimension_plugin(CreativeRangePlugin);
    }
}

/// The AI of the mobs in every dimension world, with the brains of `mcrs_minecraft_goals`; a hit
/// by a damage type of the tag `panic_causes` makes a mob panic (GOAL-PNC-002).
pub struct VanillaMobBrainPlugin;

impl Plugin for VanillaMobBrainPlugin {
    fn build(&self, app: &mut App) {
        app.add_dimension_plugin(MobBrainPlugin::new(
            vanilla_brains::vanilla_brain,
            PANIC_CAUSES,
        ));
    }
}

/// The item stacks of the player files, in the host and in every dimension world. The item table
/// comes from the registries that the server loads.
pub struct VanillaItemPlugin;

impl Plugin for VanillaItemPlugin {
    fn build(&self, app: &mut App) {
        app.add_persistence_plugin(ItemPersistencePlugin)
            .add_persistence_plugin(InventoryPersistencePlugin);
    }
}

/// Block placing in every dimension world. It reads the held item from the slots that
/// [`VanillaItemEntityPlugin`] gives each player.
pub struct VanillaBlockPlacePlugin;

impl Plugin for VanillaBlockPlacePlugin {
    fn build(&self, app: &mut App) {
        app.add_dimension_plugin(BlockPlacePlugin);
    }
}

/// Chat in every dimension world, and the operator messages of commands delivered to the
/// operators.
pub struct VanillaChatPlugin;

impl Plugin for VanillaChatPlugin {
    fn build(&self, app: &mut App) {
        app.add_dimension_plugin(ChatPlugin);
        app.add_dimension_plugin(operator_broadcast::OperatorBroadcastPlugin);
    }
}
