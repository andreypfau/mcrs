use crate::Enchantments;
use crate::Neutral;
use crate::entity_form::EntityForm;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "bevy", derive(bevy_ecs::component::Component))]
pub struct StoredEnchantments(pub Enchantments);

impl Neutral for StoredEnchantments {
    const NEUTRAL: &'static Self = &Self(Enchantments::EMPTY);
}

impl EntityForm for StoredEnchantments {
    const NEUTRAL: Option<&'static Self> = Some(<Self as Neutral>::NEUTRAL);

    fn borrow_static(&'static self) -> Self {
        Self(self.0.borrow_static())
    }
}
