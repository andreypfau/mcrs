use crate::InlineSoundEvent;
use crate::SoundEventId;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum SoundEventRef {
    Registry(SoundEventId),
    Inline(InlineSoundEvent),
}

impl SoundEventRef {
    pub(crate) fn borrow_static(&'static self) -> Self {
        match self {
            Self::Registry(id) => Self::Registry(*id),
            Self::Inline(inline) => Self::Inline(inline.borrow_static()),
        }
    }
}
