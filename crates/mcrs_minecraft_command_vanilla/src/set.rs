use bevy_ecs::prelude::{Query, Res};
use bevy_ecs::system::SystemParam;
use mcrs_minecraft_block::definition::Blocks;
use mcrs_minecraft_block::definition::schema::BlockProperty;
use mcrs_minecraft_command::{Identifier, Lookup};
use mcrs_minecraft_command_argument::TimeLookup;
use mcrs_minecraft_command_argument_block::BlockLookup;
use mcrs_minecraft_command_argument_vanilla::{ArgumentError, VanillaArgument};
use mcrs_minecraft_command_engine::{CommandContext, CommandSet, CommandTargets};
use mcrs_minecraft_command_registry::RegistryLookup;
use mcrs_minecraft_core::StaticResourceLocation;
use mcrs_minecraft_engine_world::world::dimension::DimensionTypeId;
use mcrs_minecraft_registries::CurrentRegistries;

/// The vanilla argument types, which read the registries, the block table and the players of
/// the world a line parses in.
pub struct VanillaCommands;

impl CommandSet for VanillaCommands {
    type Argument = VanillaArgument;
    type Error = ArgumentError;
    type Lookup = VanillaWorld<'static, 'static>;
}

/// What a handler of a vanilla command reads its arguments with.
pub type VanillaContext<'w, 's> = CommandContext<'w, 's, VanillaCommands>;

/// What the vanilla arguments read from a dimension world.
#[derive(SystemParam)]
pub struct VanillaWorld<'w, 's> {
    targets: Res<'w, CommandTargets>,
    registries: Option<Res<'w, CurrentRegistries>>,
    blocks: Option<Res<'w, Blocks>>,
    dimension_type: Query<'w, 's, &'static DimensionTypeId>,
}

impl VanillaWorld<'_, '_> {
    /// The registries and the block table the arguments of a command read.
    pub fn registry_lookup(&self) -> RegistryLookup<'_> {
        RegistryLookup {
            registries: self.registries.as_deref().map(|registries| &**registries),
            blocks: self.blocks.as_deref().map(|blocks| &*blocks.0),
            dimension_type: self.dimension_type.iter().next().map(|id| id.0),
        }
    }

    /// The block table of this world, when it has one.
    pub fn blocks(&self) -> Option<&Blocks> {
        self.blocks.as_deref()
    }
}

impl Lookup for VanillaWorld<'_, '_> {
    fn entries(&self, registry: StaticResourceLocation) -> impl Iterator<Item = &str> {
        self.registry_lookup().entries(registry)
    }

    fn tags(&self, registry: StaticResourceLocation) -> impl Iterator<Item = &str> {
        self.registry_lookup().tags(registry)
    }

    fn tag_contains(
        &self,
        registry: StaticResourceLocation,
        tag: Identifier<'_>,
        entry: StaticResourceLocation,
    ) -> bool {
        self.registry_lookup().tag_contains(registry, tag, entry)
    }

    fn player_names(&self) -> impl Iterator<Item = &str> {
        self.targets
            .players()
            .iter()
            .map(|player| player.name.as_str())
    }

    fn dimensions(&self) -> impl Iterator<Item = &str> {
        self.targets
            .dimensions()
            .iter()
            .map(|dimension| dimension.key.as_str())
    }

    fn tag_entries(
        &self,
        registry: StaticResourceLocation,
        tag: Identifier<'_>,
    ) -> impl Iterator<Item = &str> {
        self.registry_lookup().tag_entries(registry, tag)
    }
}

impl BlockLookup for VanillaWorld<'_, '_> {
    fn block_properties(&self, block: Identifier<'_>) -> Option<&[BlockProperty]> {
        self.registry_lookup().block_properties(block)
    }

    fn has_block_entity(&self, block: Identifier<'_>) -> bool {
        self.registry_lookup().has_block_entity(block)
    }
}

impl TimeLookup for VanillaWorld<'_, '_> {
    fn suggested_time_markers(&self, clock: Option<Identifier<'_>>) -> impl Iterator<Item = &str> {
        self.registry_lookup().suggested_time_markers(clock)
    }

    fn timelines_of_clock(&self, clock: Option<Identifier<'_>>) -> impl Iterator<Item = &str> {
        self.registry_lookup().timelines_of_clock(clock)
    }
}
