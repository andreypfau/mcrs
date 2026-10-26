use crate::command::CommandContext;
use mcrs_minecraft_command::{Identifier, Lookup, ResourceOrTag};
use mcrs_minecraft_core::{ResourceLocation, StaticResourceLocation};
use mcrs_minecraft_registries::Registries;
use std::borrow::Cow;

impl CommandContext<'_, '_> {
    fn registries(&self) -> Option<&Registries> {
        self.registries.as_deref().map(|registries| &**registries)
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
}
