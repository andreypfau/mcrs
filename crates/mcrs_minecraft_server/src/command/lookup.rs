use crate::command::CommandContext;
use mcrs_minecraft_block::definition::schema::BlockProperty;
use mcrs_minecraft_command::{Identifier, Lookup, ResourceOrTag};
use mcrs_minecraft_core::{Id, ResourceLocation, StaticResourceLocation};
use mcrs_minecraft_dimension_environment::dimension_type::DimensionTypeFile;
use mcrs_minecraft_environment::timeline::Timeline;
use mcrs_minecraft_environment::world_clock::WorldClock;
use mcrs_minecraft_registries::Registries;
use std::borrow::Cow;

impl CommandContext<'_, '_> {
    pub(crate) fn registries(&self) -> Option<&Registries> {
        self.registries.as_deref().map(|registries| &**registries)
    }

    /// The name of the dimension type of this world, and its default clock.
    pub(crate) fn dimension_clock(
        &self,
    ) -> Option<(Option<&ResourceLocation>, Option<Id<WorldClock>>)> {
        let type_id = self.dimension_type.iter().next()?.0;
        let types = self.registries()?.registry::<DimensionTypeFile>()?;
        let id = Id::checked(type_id.number(), types.len())?;
        Some((types.name(id), types.get(id)?.default_clock))
    }

    /// The clock that a time command selects: the named one, or the default clock of this
    /// world's dimension type.
    pub(crate) fn clock_id(&self, clock: Option<Identifier<'_>>) -> Option<Id<WorldClock>> {
        match clock {
            None => self.dimension_clock()?.1,
            Some(name) => self
                .registries()?
                .registry::<WorldClock>()?
                .iter()
                .find(|(_, entry, _)| name.eq_str(entry.as_str()))
                .map(|(id, _, _)| id),
        }
    }

    pub(crate) fn clock_name(&self, clock: Id<WorldClock>) -> Option<&ResourceLocation> {
        self.registries()?.registry::<WorldClock>()?.name(clock)
    }

    /// The timelines that run on the clock, with their names.
    pub(crate) fn timelines(
        &self,
        clock: Option<Id<WorldClock>>,
    ) -> impl Iterator<Item = (&ResourceLocation, &Timeline)> {
        self.registries()
            .and_then(|registries| registries.registry::<Timeline>())
            .into_iter()
            .flat_map(|timelines| timelines.iter())
            .filter(move |(_, _, timeline)| Some(timeline.clock) == clock)
            .map(|(_, name, timeline)| (name, timeline))
    }

    pub(crate) fn timeline(&self, name: Identifier<'_>) -> Option<(&ResourceLocation, &Timeline)> {
        self.registries()?
            .registry::<Timeline>()?
            .iter()
            .find(|(_, entry, _)| name.eq_str(entry.as_str()))
            .map(|(_, name, timeline)| (name, timeline))
    }

    fn names(&self, registry: StaticResourceLocation) -> &[ResourceLocation] {
        self.registries()
            .and_then(|registries| registries.names_of(&registry))
            .unwrap_or_default()
    }

    fn tag_members(&self, registry: StaticResourceLocation, tag: Identifier<'_>) -> Option<&[u16]> {
        self.registries()?
            .tags_of(&registry)?
            .find(|(name, _)| tag.eq_str(name.as_str()))
            .map(|(_, members)| members)
    }

    pub(crate) fn name(
        &self,
        registry: StaticResourceLocation,
        id: u16,
    ) -> Option<&ResourceLocation> {
        self.names(registry).get(usize::from(id))
    }

    /// The registry ids of an entry or of the members of a tag.
    pub(crate) fn ids(
        &self,
        registry: StaticResourceLocation,
        value: ResourceOrTag<'_>,
    ) -> Option<Cow<'_, [u16]>> {
        match value {
            ResourceOrTag::Entry(id) => {
                let names = self.names(registry);
                let index = names.iter().position(|name| id.eq_str(name.as_str()))?;
                Some(Cow::Owned(vec![u16::try_from(index).ok()?]))
            }
            ResourceOrTag::Tag(tag) => Some(Cow::Borrowed(self.tag_members(registry, tag)?)),
        }
    }
}

impl Lookup for CommandContext<'_, '_> {
    fn entries(&self, registry: StaticResourceLocation) -> impl Iterator<Item = &str> {
        self.names(registry).iter().map(ResourceLocation::as_str)
    }

    fn tags(&self, registry: StaticResourceLocation) -> impl Iterator<Item = &str> {
        self.registries()
            .and_then(|registries| registries.tags_of(&registry))
            .into_iter()
            .flatten()
            .map(|(name, _)| name.as_str())
    }

    fn tag_contains(
        &self,
        registry: StaticResourceLocation,
        tag: Identifier<'_>,
        entry: StaticResourceLocation,
    ) -> bool {
        let Some(index) = self
            .registries()
            .and_then(|registries| registries.id_of(&registry, entry.as_str()))
        else {
            return false;
        };
        self.tag_members(registry, tag)
            .is_some_and(|members| members.contains(&index))
    }

    fn player_names(&self) -> impl Iterator<Item = &str> {
        self.profiles
            .iter()
            .map(|profile| profile.username.as_str())
    }

    fn tag_entries(
        &self,
        registry: StaticResourceLocation,
        tag: Identifier<'_>,
    ) -> impl Iterator<Item = &str> {
        let names = self.names(registry);
        self.tag_members(registry, tag)
            .unwrap_or_default()
            .iter()
            .filter_map(move |&id| names.get(usize::from(id)))
            .map(ResourceLocation::as_str)
    }

    // chisle: a linear scan of the block table per read; an index by identifier that takes
    // borrowed text lifts it.
    fn block_properties(&self, block: Identifier<'_>) -> Option<&[BlockProperty]> {
        self.blocks
            .as_deref()?
            .0
            .blocks()
            .iter()
            .find(|entry| block.eq_str(entry.identifier.as_str()))
            .map(|entry| entry.properties.0.as_slice())
    }

    fn suggested_time_markers(&self, clock: Option<Identifier<'_>>) -> impl Iterator<Item = &str> {
        self.timelines(self.clock_id(clock))
            .flat_map(|(_, timeline)| &timeline.time_markers)
            .filter(|(_, marker)| marker.show_in_commands)
            .map(|(name, _)| name.as_str())
    }

    fn timelines_of_clock(&self, clock: Option<Identifier<'_>>) -> impl Iterator<Item = &str> {
        self.timelines(self.clock_id(clock))
            .map(|(name, _)| name.as_str())
    }
}
