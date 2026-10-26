use crate::identifier::Identifier;
use crate::permission::CommandLevel;
use glam::DVec3;
use mcrs_minecraft_core::StaticResourceLocation;
use mcrs_minecraft_protocol::packets::game::clientbound::LookAnchor;

/// What the arguments read from the world of the source: registries, players and dimensions.
pub trait Lookup {
    /// The identifiers of the entries, each as `namespace:path`.
    fn entries(&self, registry: StaticResourceLocation) -> impl Iterator<Item = &str>;

    fn tags(&self, registry: StaticResourceLocation) -> impl Iterator<Item = &str>;

    fn has_entry(&self, registry: StaticResourceLocation, entry: Identifier<'_>) -> bool {
        self.entries(registry).any(|name| entry.eq_str(name))
    }

    fn has_tag(&self, registry: StaticResourceLocation, tag: Identifier<'_>) -> bool {
        self.tags(registry).any(|name| tag.eq_str(name))
    }

    fn tag_contains(
        &self,
        registry: StaticResourceLocation,
        tag: Identifier<'_>,
        entry: StaticResourceLocation,
    ) -> bool;

    fn player_names(&self) -> impl Iterator<Item = &str>;
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
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Anchor {
    #[default]
    Feet,
    Eyes,
}

impl From<Anchor> for LookAnchor {
    fn from(anchor: Anchor) -> Self {
        match anchor {
            Anchor::Feet => LookAnchor::Feet,
            Anchor::Eyes => LookAnchor::Eyes,
        }
    }
}

impl Anchor {
    pub fn apply(self, position: DVec3, eye_height: f64) -> DVec3 {
        match self {
            Anchor::Feet => position,
            Anchor::Eyes => position + DVec3::Y * eye_height,
        }
    }
}

/// The sender of a command and what the arguments read from it.
#[derive(Clone, Copy, Debug)]
pub struct Source<'s, L> {
    pub level: CommandLevel,
    pub position: DVec3,
    pub yaw: f32,
    pub pitch: f32,
    pub anchor: Anchor,
    /// `None` when the source is no entity.
    pub eye_height: Option<f64>,
    pub lookup: &'s L,
}

impl<'s, L> Source<'s, L> {
    pub const fn new(level: CommandLevel, lookup: &'s L) -> Self {
        Self {
            level,
            position: DVec3::ZERO,
            yaw: 0.0,
            pitch: 0.0,
            anchor: Anchor::Feet,
            eye_height: None,
            lookup,
        }
    }

    pub(crate) fn anchor_position(&self) -> DVec3 {
        self.eye_height.map_or(self.position, |eye_height| {
            self.anchor.apply(self.position, eye_height)
        })
    }
}
