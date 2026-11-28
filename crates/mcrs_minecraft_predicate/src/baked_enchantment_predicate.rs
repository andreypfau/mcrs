use fixedbitset::FixedBitSet;
use mcrs_minecraft_item_stack::Enchantments;
use mcrs_minecraft_registries::Registries;

use crate::id_bits::id_bits;
use crate::predicate_issue::PredicateIssue;
use mcrs_minecraft_item::component::common::MinMaxBounds;
use mcrs_minecraft_item::component::predicate::EnchantmentPredicate;

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct BakedEnchantmentPredicate {
    enchantments: Option<FixedBitSet>,
    levels: MinMaxBounds<i32>,
}

impl BakedEnchantmentPredicate {
    pub(crate) fn bake(
        predicate: &EnchantmentPredicate,
        registries: &Registries,
    ) -> Result<Self, PredicateIssue> {
        Ok(Self {
            enchantments: predicate
                .enchantments
                .as_ref()
                .map(|set| id_bits(set, registries, PredicateIssue::UnloadedEnchantmentTags))
                .transpose()?,
            levels: predicate.levels,
        })
    }

    pub(crate) fn matches(&self, component: &Enchantments) -> bool {
        component.entries().iter().any(|entry| {
            self.enchantments
                .as_ref()
                .is_none_or(|bits| bits.contains(usize::from(entry.enchantment.0)))
                && self.levels.matches(i32::from(entry.level.get()))
        })
    }
}
