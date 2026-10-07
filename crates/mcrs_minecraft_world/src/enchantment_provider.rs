use mcrs_minecraft_core::codec::Bounded;
use mcrs_minecraft_item::enchantment::EnchantmentData;
use mcrs_minecraft_registry::{HolderSet, Id};
use mcrs_minecraft_value_provider::IntProvider;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(remote = "Self", deny_unknown_fields)]
pub enum EnchantmentProvider {
    ByCost {
        enchantments: HolderSet<EnchantmentData>,
        cost: IntProvider,
    },
    ByCostWithDifficulty {
        enchantments: HolderSet<EnchantmentData>,
        min_cost: Bounded<1, 10000>,
        max_cost_span: Bounded<0, 10000>,
    },
    Single {
        enchantment: Id<EnchantmentData>,
        level: IntProvider,
    },
}

mcrs_minecraft_registry::dispatch! {
    EnchantmentProvider, key = "type", registry = mcrs_minecraft_enchantment::keys::EnchantmentProviderType,
    {
        ByCost => ByCost,
        ByCostWithDifficulty => ByCostWithDifficulty,
        Single => Single,
    }
}
