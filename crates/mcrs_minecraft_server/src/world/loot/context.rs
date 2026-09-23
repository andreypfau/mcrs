use crate::world::loot::condition::LootCondition;
use mcrs_minecraft_core::ResourceLocation;
use mcrs_minecraft_protocol::item::Enchantments;

pub struct BlockBreakContext<'a> {
    pub tool_enchantments: Option<&'a Enchantments>,
}

#[derive(Debug, Clone)]
pub struct LootDrop {
    pub item_name: ResourceLocation,
    pub count: u8,
}

impl LootCondition {
    pub fn check(&self, ctx: &BlockBreakContext) -> bool {
        match self {
            LootCondition::MatchToolEnchantment {
                enchantment,
                min_level,
            } => ctx.tool_enchantments.is_some_and(|enchantments| {
                enchantments.level(enchantment) >= i32::from(*min_level)
            }),
            LootCondition::SurvivesExplosion => true,
            LootCondition::Inverted(inner) => !inner.check(ctx),
            LootCondition::AnyOf(conditions) => conditions.iter().any(|c| c.check(ctx)),
            LootCondition::AllOf(conditions) => conditions.iter().all(|c| c.check(ctx)),
            LootCondition::AlwaysTrue => true,
        }
    }
}
