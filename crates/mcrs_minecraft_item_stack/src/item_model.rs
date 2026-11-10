use mcrs_minecraft_resource::ResourceLocation;
use mcrs_minecraft_resource::ResourceLocationBuf;

use crate::identifier::Identifier;

/// The item model of the stack in a resource pack. Nothing checks that the model exists.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "bevy", derive(bevy_ecs::component::Component))]
pub struct ItemModel(Identifier);

impl ItemModel {
    #[must_use]
    pub const fn new(model: ResourceLocationBuf) -> Self {
        Self(Identifier::Owned(model))
    }

    #[must_use]
    pub const fn from_static(model: ResourceLocation<'static>) -> Self {
        Self(Identifier::Borrowed(model))
    }

    #[must_use]
    pub fn get(&self) -> ResourceLocation<'_> {
        self.0.as_location()
    }
}
