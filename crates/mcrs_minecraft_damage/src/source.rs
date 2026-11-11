use glam::DVec3;
use mcrs_minecraft_entity_key::EntityKey;

/// The cause of one hit (DMG-SRC-001).
#[derive(Clone, Copy, Debug, PartialEq)]
#[non_exhaustive]
pub struct DamageSource {
    pub damage_type: u32,
    pub causing_entity: Option<EntityKey>,
    pub direct_entity: Option<EntityKey>,
    pub source_position: Option<DVec3>,
}

impl DamageSource {
    #[must_use]
    pub const fn new(damage_type: u32) -> Self {
        Self {
            damage_type,
            causing_entity: None,
            direct_entity: None,
            source_position: None,
        }
    }

    #[must_use]
    pub const fn with_causing(mut self, entity: EntityKey) -> Self {
        self.causing_entity = Some(entity);
        self
    }

    #[must_use]
    pub const fn with_direct(mut self, entity: EntityKey) -> Self {
        self.direct_entity = Some(entity);
        self
    }

    #[must_use]
    pub const fn with_position(mut self, position: DVec3) -> Self {
        self.source_position = Some(position);
        self
    }
}
