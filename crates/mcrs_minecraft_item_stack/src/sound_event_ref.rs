use crate::InlineSoundEvent;
use crate::RegistryValue;
use crate::SoundEventId;

pub type SoundEventRef = RegistryValue<SoundEventId, InlineSoundEvent>;

impl SoundEventRef {
    pub(crate) fn borrow_static(&'static self) -> Self {
        match self {
            Self::Reference(id) => Self::Reference(*id),
            Self::Inline(inline) => Self::Inline(inline.borrow_static()),
        }
    }
}
