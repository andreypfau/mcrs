use crate::SoundEventRef;
use crate::entity_form::EntityForm;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "bevy", derive(bevy_ecs::component::Component))]
pub struct PiercingWeapon {
    pub deals_knockback: bool,
    pub dismounts: bool,
    pub sound: Option<SoundEventRef>,
    pub hit_sound: Option<SoundEventRef>,
}

impl PiercingWeapon {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            deals_knockback: true,
            dismounts: false,
            sound: None,
            hit_sound: None,
        }
    }
}

impl Default for PiercingWeapon {
    fn default() -> Self {
        Self::new()
    }
}

impl EntityForm for PiercingWeapon {
    fn borrow_static(&'static self) -> Self {
        Self {
            deals_knockback: self.deals_knockback,
            dismounts: self.dismounts,
            sound: self.sound.clone(),
            hit_sound: self.hit_sound.clone(),
        }
    }
}
