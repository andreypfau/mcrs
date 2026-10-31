use mcrs_minecraft_block::definition::schema::BlockProperty;
use mcrs_minecraft_block::definition::{BlockDefinitions, BlockEntry, BlockStateFlags};
use mcrs_minecraft_command::Identifier;
use mcrs_minecraft_command_argument::ResourceOrTag;
use mcrs_minecraft_dimension::DimensionType;
use mcrs_minecraft_dimension_environment::dimension_type::DimensionTypeFile;
use mcrs_minecraft_environment::timeline::Timeline;
use mcrs_minecraft_environment::world_clock::WorldClock;
use mcrs_minecraft_registries::Registries;
use mcrs_minecraft_resource::{Id, ResourceLocation, ResourceLocationBuf, StaticResourceLocation};

/// The dimension type of the source and its default clock.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DimensionClock<'a> {
    pub dimension_type: Option<&'a ResourceLocationBuf>,
    pub clock: Option<Id<WorldClock>>,
}

/// The registries and the block table a host lends to its command lookup.
#[derive(Clone, Copy, Default)]
pub struct RegistryLookup<'a> {
    pub registries: Option<&'a Registries>,
    pub blocks: Option<&'a BlockDefinitions>,
    /// The dimension type of the source dimension, whose clock is the default clock.
    pub dimension_type: Option<Id<DimensionType>>,
}

impl<'a> RegistryLookup<'a> {
    /// Whether the registries hold `registry`.
    pub fn has_registry(&self, registry: StaticResourceLocation) -> bool {
        self.registries
            .is_some_and(|registries| registries.names_of(&registry).is_some())
    }

    /// The names of the entries of `registry` in id order.
    pub fn names(&self, registry: StaticResourceLocation) -> &'a [ResourceLocationBuf] {
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

    /// The identifiers of the tags of `registry`, each as `namespace:path`.
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

    /// The registry ids of the members of `tag`.
    pub fn tag_members(
        &self,
        registry: StaticResourceLocation,
        tag: Identifier<'_>,
    ) -> Option<&'a [u16]> {
        self.registries?
            .tags_of(&registry)?
            .find(|(name, _)| tag == name.as_str())
            .map(|(_, members)| members)
    }

    /// Whether `tag` of `registry` holds `entry`.
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

    /// The identifiers of the members of `tag`, each as `namespace:path`.
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
            .position(|entry| name == entry.as_str())?;
        u16::try_from(index).ok()
    }

    /// The entry of the block table for `block`.
    // chisle: a linear scan of the block table per read; an index by identifier that takes
    // borrowed text lifts it.
    pub fn block(&self, block: Identifier<'_>) -> Option<&'a BlockEntry> {
        self.blocks?
            .blocks()
            .iter()
            .find(|entry| block == entry.identifier.as_str())
    }

    /// The properties of `block` in their order.
    pub fn block_properties(&self, block: Identifier<'_>) -> Option<&'a [BlockProperty]> {
        self.block(block).map(|entry| entry.properties.0.as_slice())
    }

    /// Whether the states of `block` have a block entity.
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

    /// The name of the entry with the registry id `id`.
    pub fn name(
        &self,
        registry: StaticResourceLocation,
        id: u16,
    ) -> Option<&'a ResourceLocationBuf> {
        self.names(registry).get(usize::from(id))
    }

    /// The name of the source dimension type, and its default clock.
    pub fn dimension_clock(&self) -> Option<DimensionClock<'a>> {
        let types = self.registries?.registry::<DimensionTypeFile>()?;
        let id = Id::checked(self.dimension_type?.number(), types.len())?;
        Some(DimensionClock {
            dimension_type: types.name(id),
            clock: types.get(id)?.default_clock,
        })
    }

    /// The named clock, or the default clock of the source dimension for `None`.
    pub fn clock(&self, name: Option<Identifier<'_>>) -> Option<Id<WorldClock>> {
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

    /// The name of a world clock.
    pub fn clock_name(&self, clock: Id<WorldClock>) -> Option<&'a ResourceLocationBuf> {
        self.registries?.registry::<WorldClock>()?.name(clock)
    }

    /// The timelines that run on the clock, with their names.
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

    /// The timeline `name`, with its name.
    pub fn timeline(
        &self,
        name: Identifier<'_>,
    ) -> Option<(&'a ResourceLocationBuf, &'a Timeline)> {
        self.registries?
            .registry::<Timeline>()?
            .iter()
            .find(|(_, entry, _)| name == entry.as_str())
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

    /// The identifiers of the timelines that run on a clock; `None` is the default clock.
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
    /// The id of one entry.
    One([u16; 1]),
    /// The ids of the members of a tag.
    Members(&'a [u16]),
}

impl Ids<'_> {
    /// The ids as a slice.
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
