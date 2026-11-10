use bevy_ecs::prelude::{Query, Res};
use bevy_ecs::system::SystemParam;
use mcrs_minecraft_block::definition::Blocks;
use mcrs_minecraft_block::definition::schema::BlockProperty;
use mcrs_minecraft_command::Lookup;
use mcrs_minecraft_command_argument::TimeLookup;
use mcrs_minecraft_command_argument_block::BlockLookup;
use mcrs_minecraft_command_argument_vanilla::{ArgumentError, VanillaArgument};
use mcrs_minecraft_command_engine::{CommandContext, CommandSet, CommandTargets};
use mcrs_minecraft_command_registry::RegistryLookup;
use mcrs_minecraft_engine::banlist::BanList;
use mcrs_minecraft_engine::command_handoff::SourceMob;
use mcrs_minecraft_engine_brain::EntityRows;
use mcrs_minecraft_engine_physics::{MobUuids, Mobs};
use mcrs_minecraft_engine_world::world::dimension::DimensionTypeId;
use mcrs_minecraft_entity_rows::Placed;
use mcrs_minecraft_registries::CurrentRegistries;
use mcrs_minecraft_resource::ResourceLocation;

use crate::mob_view::MobView;

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
    bans: Option<Res<'w, BanList>>,
    registries: Option<Res<'w, CurrentRegistries>>,
    blocks: Option<Res<'w, Blocks>>,
    dimension_type: Query<'w, 's, &'static DimensionTypeId>,
    rows: Option<Res<'w, EntityRows>>,
    mobs: Option<Res<'w, Mobs>>,
    uuids: Option<Res<'w, MobUuids>>,
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

    /// Each mob row of this world.
    pub(crate) fn mobs(&self) -> impl Iterator<Item = MobView<'_>> {
        let rows = self.rows.as_deref();
        let mobs = self.mobs.as_deref();
        rows.zip(mobs).into_iter().flat_map(|(rows, mobs)| {
            rows.living_rows().filter_map(|(at, state)| {
                Some(MobView {
                    at,
                    state,
                    mob: mobs.0.get(&at.id)?,
                })
            })
        })
    }

    /// The mob row that a source holds; `None` when it left the world or its tile.
    pub(crate) fn mob(&self, source: SourceMob) -> Option<MobView<'_>> {
        let at = Placed {
            tile: source.tile,
            id: source.id,
        };
        Some(MobView {
            at,
            state: self.rows.as_deref()?.state_of(at)?,
            mob: self.mobs.as_deref()?.0.get(&source.id)?,
        })
    }

    /// The mob row of this world with the UUID `uuid`.
    pub(crate) fn mob_by_uuid(&self, uuid: u128) -> Option<MobView<'_>> {
        let id = *self.uuids.as_deref()?.0.get(&uuid)?;
        let mob = self.mobs.as_deref()?.0.get(&id)?;
        let rows = self.rows.as_deref()?;
        let at = Placed {
            tile: rows.tile_of(id)?,
            id,
        };
        Some(MobView {
            at,
            state: rows.state_of(at)?,
            mob,
        })
    }

    /// The block table of this world, when it has one.
    pub fn blocks(&self) -> Option<&Blocks> {
        self.blocks.as_deref()
    }
}

impl Lookup for VanillaWorld<'_, '_> {
    fn entries(&self, registry: ResourceLocation<'static>) -> impl Iterator<Item = &str> {
        self.registry_lookup().entries(registry)
    }

    fn tags(&self, registry: ResourceLocation<'static>) -> impl Iterator<Item = &str> {
        self.registry_lookup().tags(registry)
    }

    fn tag_contains(
        &self,
        registry: ResourceLocation<'static>,
        tag: ResourceLocation<'_>,
        entry: ResourceLocation<'static>,
    ) -> bool {
        self.registry_lookup().tag_contains(registry, tag, entry)
    }

    fn player_names(&self) -> impl Iterator<Item = &str> {
        self.targets
            .players()
            .iter()
            .map(|player| player.name.as_str())
    }

    fn banned_names(&self) -> impl Iterator<Item = &str> {
        self.bans
            .as_deref()
            .into_iter()
            .flat_map(BanList::entries)
            .map(|entry| entry.name.as_str())
            .filter(|name| !name.is_empty())
    }

    fn dimensions(&self) -> impl Iterator<Item = &str> {
        self.targets
            .dimensions()
            .iter()
            .map(|dimension| dimension.key.as_str())
    }

    fn tag_entries(
        &self,
        registry: ResourceLocation<'static>,
        tag: ResourceLocation<'_>,
    ) -> impl Iterator<Item = &str> {
        self.registry_lookup().tag_entries(registry, tag)
    }
}

impl BlockLookup for VanillaWorld<'_, '_> {
    fn block_properties(&self, block: ResourceLocation<'_>) -> Option<&[BlockProperty]> {
        self.registry_lookup().block_properties(block)
    }

    fn has_block_entity(&self, block: ResourceLocation<'_>) -> bool {
        self.registry_lookup().has_block_entity(block)
    }
}

impl TimeLookup for VanillaWorld<'_, '_> {
    fn suggested_time_markers(
        &self,
        clock: Option<ResourceLocation<'_>>,
    ) -> impl Iterator<Item = &str> {
        self.registry_lookup().suggested_time_markers(clock)
    }

    fn timelines_of_clock(
        &self,
        clock: Option<ResourceLocation<'_>>,
    ) -> impl Iterator<Item = &str> {
        self.registry_lookup().timelines_of_clock(clock)
    }
}
