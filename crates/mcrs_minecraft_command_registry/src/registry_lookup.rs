use mcrs_minecraft_block::definition::schema::BlockProperty;
use mcrs_minecraft_block::definition::{BlockDefinitions, BlockEntry, BlockStateFlags};
use mcrs_minecraft_command_argument::ResourceOrTag;
use mcrs_minecraft_dimension::DimensionType;
use mcrs_minecraft_dimension_environment::dimension_type::DimensionTypeFile;
use mcrs_minecraft_environment::Timeline;
use mcrs_minecraft_environment::WorldClock;
use mcrs_minecraft_registries::Registries;
use mcrs_minecraft_resource::{Id, ResourceLocation, ResourceLocationBuf};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DimensionClock<'a> {
    pub dimension_type: Option<&'a ResourceLocationBuf>,
    pub clock: Option<Id<WorldClock>>,
}

#[derive(Clone, Copy, Default)]
pub struct RegistryLookup<'a> {
    pub registries: Option<&'a Registries>,
    pub blocks: Option<&'a BlockDefinitions>,
    pub dimension_type: Option<Id<DimensionType>>,
}

impl<'a> RegistryLookup<'a> {
    pub fn has_registry(&self, registry: ResourceLocation<'static>) -> bool {
        self.registries
            .is_some_and(|registries| registries.names_of(registry).is_some())
    }

    pub fn names(&self, registry: ResourceLocation<'static>) -> &'a [ResourceLocationBuf] {
        self.registries
            .and_then(|registries| registries.names_of(registry))
            .unwrap_or_default()
    }

    pub fn entries(
        &self,
        registry: ResourceLocation<'static>,
    ) -> impl Iterator<Item = &'a str> + use<'a> {
        self.names(registry).iter().map(ResourceLocationBuf::as_str)
    }

    pub fn tags(
        &self,
        registry: ResourceLocation<'static>,
    ) -> impl Iterator<Item = &'a str> + use<'a> {
        self.registries
            .and_then(|registries| registries.tags_of(registry))
            .into_iter()
            .flatten()
            .map(|(name, _)| name.as_str())
    }

    pub fn tag_members(
        &self,
        registry: ResourceLocation<'static>,
        tag: ResourceLocation<'_>,
    ) -> Option<&'a [u16]> {
        self.registries?
            .tags_of(registry)?
            .find(|(name, _)| tag == name.as_str())
            .map(|(_, members)| members)
    }

    pub fn tag_contains(
        &self,
        registry: ResourceLocation<'static>,
        tag: ResourceLocation<'_>,
        entry: ResourceLocation<'static>,
    ) -> bool {
        self.registries
            .and_then(|registries| registries.id_of(registry, entry))
            .is_some_and(|id| {
                self.tag_members(registry, tag)
                    .is_some_and(|members| members.contains(&id))
            })
    }

    pub fn tag_entries(
        &self,
        registry: ResourceLocation<'static>,
        tag: ResourceLocation<'_>,
    ) -> impl Iterator<Item = &'a str> + use<'a> {
        let names = self.names(registry);
        self.tag_members(registry, tag)
            .unwrap_or_default()
            .iter()
            .filter_map(move |&id| names.get(usize::from(id)))
            .map(ResourceLocationBuf::as_str)
    }

    pub fn id(
        &self,
        registry: ResourceLocation<'static>,
        name: ResourceLocation<'_>,
    ) -> Option<u16> {
        let index = self
            .names(registry)
            .iter()
            .position(|entry| name == entry.as_str())?;
        u16::try_from(index).ok()
    }

    pub fn block(&self, block: ResourceLocation<'_>) -> Option<&'a BlockEntry> {
        self.blocks?
            .blocks()
            .iter()
            .find(|entry| block == entry.identifier.as_str())
    }

    pub fn block_properties(&self, block: ResourceLocation<'_>) -> Option<&'a [BlockProperty]> {
        self.block(block).map(|entry| entry.properties.0.as_slice())
    }

    pub fn has_block_entity(&self, block: ResourceLocation<'_>) -> bool {
        let (Some(blocks), Some(entry)) = (self.blocks, self.block(block)) else {
            return false;
        };
        blocks
            .state(entry.default_state_id)
            .flags
            .contains(BlockStateFlags::HAS_BLOCK_ENTITY)
    }

    pub fn ids(
        &self,
        registry: ResourceLocation<'static>,
        value: ResourceOrTag<'_>,
    ) -> Option<Ids<'a>> {
        match value {
            ResourceOrTag::Entry(name) => Some(Ids::One([self.id(registry, name)?])),
            ResourceOrTag::Tag(tag) => Some(Ids::Members(self.tag_members(registry, tag)?)),
        }
    }

    pub fn name(
        &self,
        registry: ResourceLocation<'static>,
        id: u16,
    ) -> Option<&'a ResourceLocationBuf> {
        self.names(registry).get(usize::from(id))
    }

    pub fn dimension_clock(&self) -> Option<DimensionClock<'a>> {
        let types = self.registries?.registry::<DimensionTypeFile>()?;
        let id = Id::checked(self.dimension_type?.number(), types.len())?;
        Some(DimensionClock {
            dimension_type: types.name(id),
            clock: types.get(id)?.default_clock,
        })
    }

    pub fn clock(&self, name: Option<ResourceLocation<'_>>) -> Option<Id<WorldClock>> {
        match name {
            None => self.dimension_clock()?.clock,
            Some(name) => self
                .registries?
                .registry::<WorldClock>()?
                .iter()
                .find(|(_, entry, _)| name == entry.as_str())
                .map(|(id, _, _)| id),
        }
    }

    pub fn clock_name(&self, clock: Id<WorldClock>) -> Option<&'a ResourceLocationBuf> {
        self.registries?.registry::<WorldClock>()?.name(clock)
    }

    pub fn timelines(
        &self,
        clock: Option<Id<WorldClock>>,
    ) -> impl Iterator<Item = (&'a ResourceLocationBuf, &'a Timeline)> + use<'a> {
        self.registries
            .and_then(|registries| registries.registry::<Timeline>())
            .into_iter()
            .flat_map(|timelines| timelines.iter())
            .filter(move |(_, _, timeline)| Some(timeline.clock) == clock)
            .map(|(_, name, timeline)| (name, timeline))
    }

    pub fn timeline(
        &self,
        name: ResourceLocation<'_>,
    ) -> Option<(&'a ResourceLocationBuf, &'a Timeline)> {
        self.registries?
            .registry::<Timeline>()?
            .iter()
            .find(|(_, entry, _)| name == entry.as_str())
            .map(|(_, name, timeline)| (name, timeline))
    }

    pub fn suggested_time_markers(
        &self,
        clock: Option<ResourceLocation<'_>>,
    ) -> impl Iterator<Item = &'a str> + use<'a> {
        self.timelines(self.clock(clock))
            .flat_map(|(_, timeline)| &timeline.time_markers)
            .filter(|(_, marker)| marker.show_in_commands)
            .map(|(name, _)| name.as_str())
    }

    pub fn timelines_of_clock(
        &self,
        clock: Option<ResourceLocation<'_>>,
    ) -> impl Iterator<Item = &'a str> + use<'a> {
        self.timelines(self.clock(clock))
            .map(|(name, _)| name.as_str())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ids<'a> {
    One([u16; 1]),
    Members(&'a [u16]),
}

impl Ids<'_> {
    #[must_use]
    pub fn as_slice(&self) -> &[u16] {
        match self {
            Ids::One(id) => id,
            Ids::Members(members) => members,
        }
    }
}

impl AsRef<[u16]> for Ids<'_> {
    fn as_ref(&self) -> &[u16] {
        self.as_slice()
    }
}
