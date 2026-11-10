use alloc::borrow::Cow;
use alloc::vec::Vec;

use crate::EnchantmentLevel;
use crate::Neutral;

/// The enchantments of the stack, at most one level for each enchantment.
// The entries are in the order of the enchantment ids, so the derived equality ignores the order
// in which a caller gave them.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "bevy", derive(bevy_ecs::component::Component))]
pub struct Enchantments(Cow<'static, [EnchantmentLevel]>);

impl Enchantments {
    /// Gives `None` when one enchantment occurs two times.
    #[must_use]
    pub fn new(entries: impl IntoIterator<Item = EnchantmentLevel>) -> Option<Self> {
        let mut entries: Vec<EnchantmentLevel> = entries.into_iter().collect();
        entries.sort_unstable_by_key(|entry| entry.enchantment);
        is_strictly_ascending(&entries).then_some(Self(Cow::Owned(entries)))
    }

    /// Gives `None` when the entries are not in the strict order of the enchantment ids.
    #[must_use]
    pub fn from_static(entries: &'static [EnchantmentLevel]) -> Option<Self> {
        is_strictly_ascending(entries).then_some(Self(Cow::Borrowed(entries)))
    }

    /// The entries in the order of the enchantment ids.
    #[must_use]
    pub fn entries(&self) -> &[EnchantmentLevel] {
        &self.0
    }
}

impl Default for Enchantments {
    fn default() -> Self {
        Self(Cow::Borrowed(&[]))
    }
}

impl Neutral for Enchantments {
    const NEUTRAL: &'static Self = &Self(Cow::Borrowed(&[]));
}

fn is_strictly_ascending(entries: &[EnchantmentLevel]) -> bool {
    entries.is_sorted_by(|left, right| left.enchantment < right.enchantment)
}
