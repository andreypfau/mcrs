use mcrs_minecraft_block::definition::schema::BlockProperty;
use mcrs_minecraft_block::definition::{BlockDefinitions, BlockEntry, BlockStateFlags};
use mcrs_minecraft_command::{Identifier, ResourceOrTag};
use mcrs_minecraft_core::{Id, ResourceLocation, StaticResourceLocation};
use mcrs_minecraft_dimension::DimensionType;
use mcrs_minecraft_dimension_environment::dimension_type::DimensionTypeFile;
use mcrs_minecraft_environment::timeline::Timeline;
use mcrs_minecraft_environment::world_clock::WorldClock;
use mcrs_minecraft_registries::Registries;

/// The registries and the block table a host lends to its command lookup.
#[derive(Clone, Copy, Default)]
pub struct RegistryLookup<'a> {
    pub registries: Option<&'a Registries>,
    pub blocks: Option<&'a BlockDefinitions>,
    /// The dimension type of the source dimension, whose clock is the default clock.
    pub dimension_type: Option<Id<DimensionType>>,
}

impl<'a> RegistryLookup<'a> {
    pub fn has_registry(&self, registry: StaticResourceLocation) -> bool {
        self.registries
            .is_some_and(|registries| registries.names_of(&registry).is_some())
    }

    pub fn names(&self, registry: StaticResourceLocation) -> &'a [ResourceLocation] {
        self.registries
            .and_then(|registries| registries.names_of(&registry))
            .unwrap_or_default()
    }

    /// The identifiers of the entries, each as `namespace:path`.
    pub fn entries(
        &self,
        registry: StaticResourceLocation,
    ) -> impl Iterator<Item = &'a str> + use<'a> {
        self.names(registry).iter().map(ResourceLocation::as_str)
    }

    pub fn tags(
        &self,
        registry: StaticResourceLocation,
    ) -> impl Iterator<Item = &'a str> + use<'a> {
        self.registries
            .and_then(|registries| registries.tags_of(&registry))
            .into_iter()
            .flatten()
            .map(|(name, _)| name.as_str())
    }

    pub fn tag_members(
        &self,
        registry: StaticResourceLocation,
        tag: Identifier<'_>,
    ) -> Option<&'a [u16]> {
        self.registries?
            .tags_of(&registry)?
            .find(|(name, _)| tag.eq_str(name.as_str()))
            .map(|(_, members)| members)
    }

    pub fn tag_contains(
        &self,
        registry: StaticResourceLocation,
        tag: Identifier<'_>,
        entry: StaticResourceLocation,
    ) -> bool {
        self.registries
            .and_then(|registries| registries.id_of(&registry, entry.as_str()))
            .is_some_and(|id| {
                self.tag_members(registry, tag)
                    .is_some_and(|members| members.contains(&id))
            })
    }

    pub fn tag_entries(
        &self,
        registry: StaticResourceLocation,
        tag: Identifier<'_>,
    ) -> impl Iterator<Item = &'a str> + use<'a> {
        let names = self.names(registry);
        self.tag_members(registry, tag)
            .unwrap_or_default()
            .iter()
            .filter_map(move |&id| names.get(usize::from(id)))
            .map(ResourceLocation::as_str)
    }

    /// The registry id of the entry `name`.
    // chisle: a linear scan of the names per read, because `id_of` takes the full text and a
    // short identifier would need it built; an index by `Identifier` lifts it.
    pub fn id(&self, registry: StaticResourceLocation, name: Identifier<'_>) -> Option<u16> {
        let index = self
            .names(registry)
            .iter()
            .position(|entry| name.eq_str(entry.as_str()))?;
        u16::try_from(index).ok()
    }

    // chisle: a linear scan of the block table per read; an index by identifier that takes
    // borrowed text lifts it.
    pub fn block(&self, block: Identifier<'_>) -> Option<&'a BlockEntry> {
        self.blocks?
            .blocks()
            .iter()
            .find(|entry| block.eq_str(entry.identifier.as_str()))
    }

    pub fn block_properties(&self, block: Identifier<'_>) -> Option<&'a [BlockProperty]> {
        self.block(block).map(|entry| entry.properties.0.as_slice())
    }

    pub fn has_block_entity(&self, block: Identifier<'_>) -> bool {
        let (Some(blocks), Some(entry)) = (self.blocks, self.block(block)) else {
            return false;
        };
        blocks
            .state(entry.default_state_id)
            .flags
            .contains(BlockStateFlags::HAS_BLOCK_ENTITY)
    }

    /// The registry ids of an entry or of the members of a tag.
    pub fn ids(
        &self,
        registry: StaticResourceLocation,
        value: ResourceOrTag<'_>,
    ) -> Option<Ids<'a>> {
        match value {
            ResourceOrTag::Entry(name) => Some(Ids::One([self.id(registry, name)?])),
            ResourceOrTag::Tag(tag) => Some(Ids::Members(self.tag_members(registry, tag)?)),
        }
    }

    pub fn name(&self, registry: StaticResourceLocation, id: u16) -> Option<&'a ResourceLocation> {
        self.names(registry).get(usize::from(id))
    }

    /// The name of the source dimension type, and its default clock.
    pub fn dimension_clock(
        &self,
    ) -> Option<(Option<&'a ResourceLocation>, Option<Id<WorldClock>>)> {
        let types = self.registries?.registry::<DimensionTypeFile>()?;
        let id = Id::checked(self.dimension_type?.number(), types.len())?;
        Some((types.name(id), types.get(id)?.default_clock))
    }

    /// The named clock, or the default clock of the source dimension for `None`.
    pub fn clock(&self, name: Option<Identifier<'_>>) -> Option<Id<WorldClock>> {
        match name {
            None => self.dimension_clock()?.1,
            Some(name) => self
                .registries?
                .registry::<WorldClock>()?
                .iter()
                .find(|(_, entry, _)| name.eq_str(entry.as_str()))
                .map(|(id, _, _)| id),
        }
    }

    pub fn clock_name(&self, clock: Id<WorldClock>) -> Option<&'a ResourceLocation> {
        self.registries?.registry::<WorldClock>()?.name(clock)
    }

    /// The timelines that run on the clock, with their names.
    pub fn timelines(
        &self,
        clock: Option<Id<WorldClock>>,
    ) -> impl Iterator<Item = (&'a ResourceLocation, &'a Timeline)> + use<'a> {
        self.registries
            .and_then(|registries| registries.registry::<Timeline>())
            .into_iter()
            .flat_map(|timelines| timelines.iter())
            .filter(move |(_, _, timeline)| Some(timeline.clock) == clock)
            .map(|(_, name, timeline)| (name, timeline))
    }

    pub fn timeline(&self, name: Identifier<'_>) -> Option<(&'a ResourceLocation, &'a Timeline)> {
        self.registries?
            .registry::<Timeline>()?
            .iter()
            .find(|(_, entry, _)| name.eq_str(entry.as_str()))
            .map(|(_, name, timeline)| (name, timeline))
    }

    /// The time markers of a clock that a command suggests.
    pub fn suggested_time_markers(
        &self,
        clock: Option<Identifier<'_>>,
    ) -> impl Iterator<Item = &'a str> + use<'a> {
        self.timelines(self.clock(clock))
            .flat_map(|(_, timeline)| &timeline.time_markers)
            .filter(|(_, marker)| marker.show_in_commands)
            .map(|(name, _)| name.as_str())
    }

    pub fn timelines_of_clock(
        &self,
        clock: Option<Identifier<'_>>,
    ) -> impl Iterator<Item = &'a str> + use<'a> {
        self.timelines(self.clock(clock))
            .map(|(name, _)| name.as_str())
    }
}

/// The registry ids of an entry or of the members of a tag, without a copy of the members.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ids<'a> {
    One([u16; 1]),
    Members(&'a [u16]),
}

impl core::ops::Deref for Ids<'_> {
    type Target = [u16];

    fn deref(&self) -> &[u16] {
        match self {
            Ids::One(id) => id,
            Ids::Members(members) => members,
        }
    }
}
