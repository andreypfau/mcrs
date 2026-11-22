use crate::DamageTags;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
#[cfg_attr(feature = "bevy", derive(bevy_ecs::component::Component))]
#[non_exhaustive]
pub struct Defence {
    pub armor: f32,
    pub toughness: f32,
    pub resistance: Option<u8>,
    pub immune: DamageTags,
    pub invulnerable: bool,
}

impl Defence {
    #[must_use]
    pub const fn new(
        armor: f32,
        toughness: f32,
        resistance: Option<u8>,
        immune: DamageTags,
    ) -> Self {
        Self {
            armor,
            toughness,
            resistance,
            immune,
            invulnerable: false,
        }
    }
}
