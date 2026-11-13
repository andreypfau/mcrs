use glam::DVec3;
use mcrs_minecraft_entity_key::Cause;

/// The cause of one hit (DMG-SRC-001).
#[derive(Clone, Copy, Debug, PartialEq)]
#[non_exhaustive]
pub struct DamageSource {
    pub damage_type: u32,
    pub cause: Cause,
    pub source_position: Option<DVec3>,
}

impl DamageSource {
    #[must_use]
    pub const fn new(damage_type: u32) -> Self {
        Self {
            damage_type,
            cause: Cause::none(),
            source_position: None,
        }
    }

    #[must_use]
    pub const fn with_cause(mut self, cause: Cause) -> Self {
        self.cause = cause;
        self
    }

    #[must_use]
    pub const fn with_position(mut self, position: DVec3) -> Self {
        self.source_position = Some(position);
        self
    }
}
