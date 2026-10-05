use mcrs_minecraft_core::codec::Bounded;
use mcrs_minecraft_core::value_provider::IntProvider;
use mcrs_minecraft_keys::Enchantment;
use mcrs_minecraft_registry::{EntrySet, Id};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", deny_unknown_fields)]
pub enum EnchantmentProvider {
    #[serde(rename = "minecraft:by_cost")]
    ByCost {
        enchantments: EntrySet<Enchantment>,
        cost: IntProvider,
    },
    #[serde(rename = "minecraft:by_cost_with_difficulty")]
    ByCostWithDifficulty {
        enchantments: EntrySet<Enchantment>,
        min_cost: Bounded<1, 10000>,
        max_cost_span: Bounded<0, 10000>,
    },
    #[serde(rename = "minecraft:single")]
    Single {
        enchantment: Id<Enchantment>,
        level: IntProvider,
    },
}
