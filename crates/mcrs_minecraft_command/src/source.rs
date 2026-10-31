use crate::identifier::Identifier;
use crate::permission::CommandLevel;
use glam::DVec3;
use mcrs_minecraft_core::LookAnchor;
use mcrs_minecraft_resource::StaticResourceLocation;

/// What the arguments read from the world of the source: registries, players and dimensions.
pub trait Lookup {
    /// The identifiers of the entries, each as `namespace:path`.
    fn entries(&self, registry: StaticResourceLocation) -> impl Iterator<Item = &str>;

    /// The identifiers of the tags of the registry, each as `namespace:path`.
    fn tags(&self, registry: StaticResourceLocation) -> impl Iterator<Item = &str>;

    /// The lookup holds the entries of the registry.
    fn has_registry(&self, _registry: StaticResourceLocation) -> bool {
        true
    }

    /// The registry holds `entry`.
    fn has_entry(&self, registry: StaticResourceLocation, entry: Identifier<'_>) -> bool {
        self.entries(registry).any(|name| entry == name)
    }

    /// The registry holds the tag `tag`.
    fn has_tag(&self, registry: StaticResourceLocation, tag: Identifier<'_>) -> bool {
        self.tags(registry).any(|name| tag == name)
    }

    /// The tag `tag` of the registry holds `entry`.
    fn tag_contains(
        &self,
        registry: StaticResourceLocation,
        tag: Identifier<'_>,
        entry: StaticResourceLocation,
    ) -> bool;

    /// The names of the online players.
    fn player_names(&self) -> impl Iterator<Item = &str>;

    /// The identifiers of the dimensions that the source knows, each as `namespace:path`.
    fn dimensions(&self) -> impl Iterator<Item = &str>;

    /// The identifiers of the members of a tag, each as `namespace:path`.
    fn tag_entries(
        &self,
        registry: StaticResourceLocation,
        tag: Identifier<'_>,
    ) -> impl Iterator<Item = &str>;
}

/// A lookup that knows nothing: a source with no world.
#[derive(Clone, Copy, Debug, Default)]
pub struct NoLookup;

impl Lookup for NoLookup {
    fn entries(&self, _: StaticResourceLocation) -> impl Iterator<Item = &str> {
        core::iter::empty()
    }

    fn tags(&self, _: StaticResourceLocation) -> impl Iterator<Item = &str> {
        core::iter::empty()
    }

    fn tag_contains(
        &self,
        _: StaticResourceLocation,
        _: Identifier<'_>,
        _: StaticResourceLocation,
    ) -> bool {
        false
    }

    fn player_names(&self) -> impl Iterator<Item = &str> {
        core::iter::empty()
    }

    fn dimensions(&self) -> impl Iterator<Item = &str> {
        core::iter::empty()
    }

    fn tag_entries(
        &self,
        _: StaticResourceLocation,
        _: Identifier<'_>,
    ) -> impl Iterator<Item = &str> {
        core::iter::empty()
    }
}

/// The point of an entity at `position` that `anchor` names.
#[must_use]
pub fn anchored(anchor: LookAnchor, position: DVec3, eye_height: f64) -> DVec3 {
    match anchor {
        LookAnchor::Feet => position,
        LookAnchor::Eyes => position + DVec3::Y * eye_height,
    }
}

/// The sender of a command and what the arguments read from it.
#[derive(Clone, Copy, Debug)]
pub struct Source<'s, L> {
    pub level: CommandLevel,
    pub position: DVec3,
    pub yaw: f32,
    pub pitch: f32,
    pub anchor: LookAnchor,
    /// `None` when the source is no entity.
    pub eye_height: Option<f64>,
    /// The position of the source dimension in the dimension order.
    pub dimension: usize,
    pub lookup: &'s L,
}

impl<'s, L> Source<'s, L> {
    pub const fn new(level: CommandLevel, lookup: &'s L) -> Self {
        Self {
            level,
            position: DVec3::ZERO,
            yaw: 0.0,
            pitch: 0.0,
            anchor: LookAnchor::Feet,
            eye_height: None,
            dimension: 0,
            lookup,
        }
    }

    pub fn anchor_position(&self) -> DVec3 {
        self.eye_height.map_or(self.position, |eye_height| {
            anchored(self.anchor, self.position, eye_height)
        })
    }
}
