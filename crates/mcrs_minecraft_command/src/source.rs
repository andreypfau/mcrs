use crate::permission::CommandLevel;
use glam::DVec3;
use mcrs_minecraft_core::LookAnchor;
use mcrs_minecraft_resource::ResourceLocation;

pub trait Lookup {
    fn banned_names(&self) -> impl Iterator<Item = &str> {
        core::iter::empty()
    }

    fn entries(&self, registry: ResourceLocation<'static>) -> impl Iterator<Item = &str>;

    fn tags(&self, registry: ResourceLocation<'static>) -> impl Iterator<Item = &str>;

    fn has_registry(&self, _registry: ResourceLocation<'static>) -> bool {
        true
    }

    fn has_entry(&self, registry: ResourceLocation<'static>, entry: ResourceLocation<'_>) -> bool {
        self.entries(registry).any(|name| entry == name)
    }

    fn has_tag(&self, registry: ResourceLocation<'static>, tag: ResourceLocation<'_>) -> bool {
        self.tags(registry).any(|name| tag == name)
    }

    fn tag_contains(
        &self,
        registry: ResourceLocation<'static>,
        tag: ResourceLocation<'_>,
        entry: ResourceLocation<'static>,
    ) -> bool;

    fn player_names(&self) -> impl Iterator<Item = &str>;

    fn dimensions(&self) -> impl Iterator<Item = &str>;

    fn tag_entries(
        &self,
        registry: ResourceLocation<'static>,
        tag: ResourceLocation<'_>,
    ) -> impl Iterator<Item = &str>;
}

#[derive(Clone, Copy, Debug, Default)]
pub struct NoLookup;

impl Lookup for NoLookup {
    fn entries(&self, _: ResourceLocation<'static>) -> impl Iterator<Item = &str> {
        core::iter::empty()
    }

    fn tags(&self, _: ResourceLocation<'static>) -> impl Iterator<Item = &str> {
        core::iter::empty()
    }

    fn tag_contains(
        &self,
        _: ResourceLocation<'static>,
        _: ResourceLocation<'_>,
        _: ResourceLocation<'static>,
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
        _: ResourceLocation<'static>,
        _: ResourceLocation<'_>,
    ) -> impl Iterator<Item = &str> {
        core::iter::empty()
    }
}

#[must_use]
pub fn anchored(anchor: LookAnchor, position: DVec3, eye_height: f64) -> DVec3 {
    match anchor {
        LookAnchor::Feet => position,
        LookAnchor::Eyes => position + DVec3::Y * eye_height,
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Source<'s, L> {
    pub level: CommandLevel,
    pub position: DVec3,
    pub yaw: f32,
    pub pitch: f32,
    pub anchor: LookAnchor,
    pub eye_height: Option<f64>,
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
