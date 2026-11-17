use core::hash::Hash;
use core::hash::Hasher;

use mcrs_minecraft_resource::ResourceLocation;
use mcrs_minecraft_resource::ResourceLocationBuf;

use crate::float_key;
use crate::identifier::Identifier;

#[derive(Clone, Debug)]
pub struct InlineSoundEvent {
    sound_id: Identifier,
    pub range: Option<f32>,
}

impl InlineSoundEvent {
    #[must_use]
    pub const fn new(sound_id: ResourceLocationBuf) -> Self {
        Self {
            sound_id: Identifier::Owned(sound_id),
            range: None,
        }
    }

    #[must_use]
    pub fn sound_id(&self) -> ResourceLocation<'_> {
        self.sound_id.as_location()
    }

    pub(crate) fn borrow_static(&'static self) -> Self {
        Self {
            sound_id: self.sound_id.borrow_static(),
            range: self.range,
        }
    }
}

impl PartialEq for InlineSoundEvent {
    fn eq(&self, other: &Self) -> bool {
        self.sound_id == other.sound_id
            && self.range.map(float_key::of_f32) == other.range.map(float_key::of_f32)
    }
}

impl Eq for InlineSoundEvent {}

impl Hash for InlineSoundEvent {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.sound_id.hash(state);
        self.range.map(float_key::of_f32).hash(state);
    }
}
