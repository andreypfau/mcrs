use bevy_ecs::entity::Entity;
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
use mcrs_minecraft_damage::Vitals;
use mcrs_minecraft_engine::banlist::BanList;
use mcrs_minecraft_engine::command_handoff::SourceMob;
use mcrs_minecraft_engine_entity_motion::MotionId;
use mcrs_minecraft_engine_entity_motion::MotionState;
use mcrs_minecraft_engine_physics::Mob;
use mcrs_minecraft_engine_physics::MobIndex;
use mcrs_minecraft_engine_world::world::dimension::DimensionTypeId;
use mcrs_minecraft_entity_key::EntityUuid;
use mcrs_minecraft_entity_key::TileKey;
use mcrs_minecraft_registries::CurrentRegistries;
use mcrs_minecraft_resource::ResourceLocation;

use crate::mob_view::MobView;

pub struct VanillaCommands;

impl CommandSet for VanillaCommands {
    type Argument = VanillaArgument;
    type Error = ArgumentError;
    type Lookup = VanillaWorld<'static, 'static>;
}

pub type VanillaContext<'w, 's> = CommandContext<'w, 's, VanillaCommands>;

#[derive(SystemParam)]
pub struct VanillaWorld<'w, 's> {
    targets: Res<'w, CommandTargets>,
    bans: Option<Res<'w, BanList>>,
    registries: Option<Res<'w, CurrentRegistries>>,
    blocks: Option<Res<'w, Blocks>>,
    dimension_type: Query<'w, 's, &'static DimensionTypeId>,
    mobs: Query<'w, 's, MobItem>,
    index: Option<Res<'w, MobIndex>>,
}

type MobItem = (
    Entity,
    &'static MotionId,
    &'static MotionState,
    Option<&'static Vitals>,
    &'static Mob,
);

fn mob_view<'a>(
    (entity, id, state, vitals, mob): (
        Entity,
        &MotionId,
        &'a MotionState,
        Option<&'a Vitals>,
        &'a Mob,
    ),
) -> MobView<'a> {
    MobView {
        entity,
        id: id.0,
        state: &state.0,
        vitals,
        mob,
    }
}

impl VanillaWorld<'_, '_> {
    pub fn registry_lookup(&self) -> RegistryLookup<'_> {
        RegistryLookup {
            registries: self.registries.as_deref().map(|registries| &**registries),
            blocks: self.blocks.as_deref().map(|blocks| &*blocks.0),
            dimension_type: self.dimension_type.iter().next().map(|id| id.0),
        }
    }

    pub(crate) fn mobs(&self) -> impl Iterator<Item = MobView<'_>> {
        let mut views: Vec<MobView<'_>> = self
            .mobs
            .iter()
            .map(mob_view)
            .filter(|view| view.state.living.is_some())
            .collect();
        views.sort_unstable_by_key(|view| (TileKey::at(view.position()), view.id));
        views.into_iter()
    }

    pub(crate) fn mob(&self, source: SourceMob) -> Option<MobView<'_>> {
        let view = self.mobs.get(source.entity).ok().map(mob_view)?;
        (view.id == source.id).then_some(view)
    }

    pub(crate) fn mob_by_uuid(&self, uuid: u128) -> Option<MobView<'_>> {
        let entity = self.index.as_deref()?.get(EntityUuid::from_u128(uuid))?;
        self.mobs.get(entity).ok().map(mob_view)
    }

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
