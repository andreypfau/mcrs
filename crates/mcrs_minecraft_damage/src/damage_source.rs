use glam::DVec3;
use mcrs_minecraft_entity_key::Cause;

#[derive(Clone, Copy, Debug, PartialEq)]
#[non_exhaustive]
pub struct DamageSource {
    pub damage_type: u32,
    pub cause: Cause,
    pub position: Option<DVec3>,
}

impl DamageSource {
    #[must_use]
    pub const fn of(damage_type: u32) -> Self {
        Self {
            damage_type,
            cause: Cause {
                causing: None,
                direct: None,
            },
            position: None,
        }
    }
}
