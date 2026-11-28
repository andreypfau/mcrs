use alloc::borrow::Cow;
use alloc::vec::Vec;

use crate::EnchantmentLevel;
use crate::Neutral;
use crate::entity_form::EntityForm;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "bevy", derive(bevy_ecs::component::Component))]
pub struct Enchantments(Cow<'static, [EnchantmentLevel]>);

impl Enchantments {
    pub(crate) const EMPTY: Self = Self(Cow::Borrowed(&[]));

    #[must_use]
    pub fn new(entries: impl IntoIterator<Item = EnchantmentLevel>) -> Option<Self> {
        let mut entries: Vec<EnchantmentLevel> = entries.into_iter().collect();
        entries.sort_unstable_by_key(|entry| entry.enchantment);
        is_strictly_ascending(&entries).then_some(Self(Cow::Owned(entries)))
    }

    #[must_use]
    pub fn entries(&self) -> &[EnchantmentLevel] {
        &self.0
    }
}

impl Neutral for Enchantments {
    const NEUTRAL: &'static Self = &Self::EMPTY;
}

impl EntityForm for Enchantments {
    const NEUTRAL: Option<&'static Self> = Some(<Self as Neutral>::NEUTRAL);

    fn borrow_static(&'static self) -> Self {
        Self(Cow::Borrowed(&self.0))
    }
}

fn is_strictly_ascending(entries: &[EnchantmentLevel]) -> bool {
    entries.is_sorted_by(|left, right| left.enchantment < right.enchantment)
}
