use alloc::borrow::Cow;

use crate::AttributeModifier;
use crate::Neutral;

#[derive(Clone, Debug, Default, PartialEq, Hash)]
#[cfg_attr(feature = "bevy", derive(bevy_ecs::component::Component))]
pub struct AttributeModifiers {
    pub modifiers: Cow<'static, [AttributeModifier]>,
}

impl Neutral for AttributeModifiers {
    const NEUTRAL: &'static Self = &Self {
        modifiers: Cow::Borrowed(&[]),
    };
}
