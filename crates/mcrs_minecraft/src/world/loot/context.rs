use mcrs_protocol::Ident;
use crate::world::item::component::Enchantments;
use crate::world::loot::condition::LootCondition;

pub struct BlockBreakContext<'a> {
    pub tool_enchantments: Option<&'a Enchantments>,
}

#[derive(Debug, Clone)]
pub struct LootDrop {
    pub item_name: Ident<String>,
    pub count: u8,
}

impl LootCondition {
    pub fn check(&self, ctx: &BlockBreakContext) -> bool {
        match self {
            LootCondition::MatchToolEnchantment {
                enchantment_registry_index,
                min_level,
            } => {
                if let Some(enchantments) = ctx.tool_enchantments {
                    enchantments.get_level_by_id(*enchantment_registry_index) >= *min_level
                } else {
                    false
                }
            }
            LootCondition::SurvivesExplosion => true,
            LootCondition::Inverted(inner) => !inner.check(ctx),
            LootCondition::AnyOf(conditions) => conditions.iter().any(|c| c.check(ctx)),
            LootCondition::AllOf(conditions) => conditions.iter().all(|c| c.check(ctx)),
            LootCondition::AlwaysTrue => true,
        }
    }
}
