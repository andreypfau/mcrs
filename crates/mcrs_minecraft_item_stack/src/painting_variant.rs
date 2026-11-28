use crate::PaintingVariantId;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "bevy", derive(bevy_ecs::component::Component))]
pub struct PaintingVariant(pub PaintingVariantId);
