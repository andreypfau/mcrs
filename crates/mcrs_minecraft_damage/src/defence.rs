use crate::DamageTypeFlags;

/// The state that reduces one hit on a living entity (DMG-HIT-001, DMG-RED-001 to DMG-RED-004).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Defence {
    pub flags: DamageTypeFlags,
    pub armor: f32,
    pub armor_toughness: f32,
    pub resistance: Option<u8>,
    pub max_absorption: f32,
    pub fire_resistance: bool,
    pub fire_immune: bool,
    pub fall_immune: bool,
    pub undead: bool,
}

impl Defence {
    #[must_use]
    pub const fn new(flags: DamageTypeFlags) -> Self {
        Self {
            flags,
            armor: 0.0,
            armor_toughness: 0.0,
            resistance: None,
            max_absorption: 0.0,
            fire_resistance: false,
            fire_immune: false,
            fall_immune: false,
            undead: false,
        }
    }
}
