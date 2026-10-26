use mcrs_minecraft_command::{Identifier, Lookup};
use mcrs_minecraft_core::{ResourceLocation, StaticResourceLocation};
use mcrs_minecraft_registries::Registries;

/// What the arguments of a command read from a dimension world: its registries and the names
/// of its players.
pub struct DimLookup<'a> {
    registries: Option<&'a Registries>,
    players: Vec<&'a str>,
}

impl<'a> DimLookup<'a> {
    pub fn new(registries: Option<&'a Registries>, players: Vec<&'a str>) -> Self {
        Self {
            registries,
            players,
        }
    }

    /// The buffer of player names, to fill again for the next lookup.
    pub fn into_names(mut self) -> Vec<&'a str> {
        self.players.clear();
        self.players
    }

    fn names(&self, registry: StaticResourceLocation) -> &[ResourceLocation] {
        self.registries
            .and_then(|registries| registries.names_of(&registry.into()))
            .unwrap_or_default()
    }

    fn tag_members(&self, registry: StaticResourceLocation, tag: Identifier<'_>) -> Option<&[u16]> {
        self.registries?
            .tags_of(&registry.into())?
            .find(|(name, _)| tag.eq_str(name.as_str()))
            .map(|(_, members)| members)
    }
}

impl Lookup for DimLookup<'_> {
    fn entries(&self, registry: StaticResourceLocation) -> impl Iterator<Item = &str> {
        self.names(registry).iter().map(ResourceLocation::as_str)
    }

    fn tags(&self, registry: StaticResourceLocation) -> impl Iterator<Item = &str> {
        let key = registry.into();
        let names: Vec<&str> = self
            .registries
            .and_then(|registries| registries.tags_of(&key))
            .into_iter()
            .flatten()
            .map(|(name, _)| name.as_str())
            .collect();
        names.into_iter()
    }

    fn tag_contains(
        &self,
        registry: StaticResourceLocation,
        tag: Identifier<'_>,
        entry: StaticResourceLocation,
    ) -> bool {
        let Some(index) = self
            .names(registry)
            .iter()
            .position(|name| *name == entry)
            .and_then(|index| u16::try_from(index).ok())
        else {
            return false;
        };
        self.tag_members(registry, tag)
            .is_some_and(|members| members.contains(&index))
    }

    fn player_names(&self) -> impl Iterator<Item = &str> {
        self.players.iter().copied()
    }
}
