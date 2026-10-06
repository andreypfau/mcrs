use mcrs_minecraft_core::codec::Bounded;
use mcrs_minecraft_core::value_provider::IntProvider;
use mcrs_minecraft_keys::Enchantment;
use mcrs_minecraft_registry::{HolderSet, Id};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", deny_unknown_fields)]
pub enum EnchantmentProvider {
    #[serde(rename = "minecraft:by_cost")]
    ByCost {
        enchantments: HolderSet<Enchantment>,
        cost: IntProvider,
    },
    #[serde(rename = "minecraft:by_cost_with_difficulty")]
    ByCostWithDifficulty {
        enchantments: HolderSet<Enchantment>,
        min_cost: Bounded<1, 10000>,
        max_cost_span: Bounded<0, 10000>,
    },
    #[serde(rename = "minecraft:single")]
    Single {
        enchantment: Id<Enchantment>,
        level: IntProvider,
    },
}

const ENCHANTMENT_PROVIDER_TYPE_ROWS: &[&str] = &[
    "minecraft:by_cost",
    "minecraft:by_cost_with_difficulty",
    "minecraft:single",
];

const _: () = assert!(mcrs_minecraft_registry::static_rows::names_cover(
    ENCHANTMENT_PROVIDER_TYPE_ROWS,
    &[],
    mcrs_minecraft_keys::enchantment_provider_type::ENTRIES
));

#[cfg(test)]
mod dispatch_rows {
    use super::*;

    #[test]
    fn enchantment_provider_type_rows_select_their_variants() {
        mcrs_minecraft_registry::static_rows::assert_dispatch::<EnchantmentProvider>(
            ENCHANTMENT_PROVIDER_TYPE_ROWS,
            &[],
            mcrs_minecraft_keys::enchantment_provider_type::ENTRIES,
            |name| serde_json::json!({ "type": name }),
        );
    }
}
