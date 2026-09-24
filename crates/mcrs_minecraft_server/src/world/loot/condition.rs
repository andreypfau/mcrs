use crate::world::loot::context::BlockBreakContext;
use mcrs_minecraft_core::ResourceLocation;
use mcrs_minecraft_item::enchantment::EnchantmentData;
use mcrs_minecraft_registry::StaticRegistry;
use serde::Deserialize;
use tracing::warn;

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "condition")]
pub enum LootCondition {
    #[serde(rename = "minecraft:match_tool")]
    MatchTool { predicate: ToolPredicate },
    #[serde(rename = "minecraft:survives_explosion")]
    SurvivesExplosion {},
    #[serde(rename = "minecraft:inverted")]
    Inverted { term: Box<LootCondition> },
    #[serde(rename = "minecraft:any_of")]
    AnyOf { terms: Vec<LootCondition> },
    #[serde(rename = "minecraft:all_of")]
    AllOf { terms: Vec<LootCondition> },
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ToolPredicate {
    #[serde(default)]
    pub predicates: Option<ToolPredicates>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ToolPredicates {
    #[serde(rename = "minecraft:enchantments", default)]
    pub enchantments: Option<Vec<EnchantmentPredicate>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct EnchantmentPredicate {
    pub enchantments: ResourceLocation,
    #[serde(default)]
    pub levels: Option<LevelRange>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LevelRange {
    #[serde(default)]
    pub min: Option<u8>,
}

impl ToolPredicate {
    fn enchantment(&self) -> Option<&EnchantmentPredicate> {
        self.predicates.as_ref()?.enchantments.as_ref()?.first()
    }
}

impl LootCondition {
    pub fn check(&self, ctx: &BlockBreakContext) -> bool {
        match self {
            LootCondition::MatchTool { predicate } => {
                let Some(required) = predicate.enchantment() else {
                    return true;
                };
                let min_level = required.levels.as_ref().and_then(|l| l.min).unwrap_or(1);
                ctx.tool_enchantments.is_some_and(|enchantments| {
                    enchantments.level(&required.enchantments) >= i32::from(min_level)
                })
            }
            LootCondition::SurvivesExplosion {} => true,
            LootCondition::Inverted { term } => !term.check(ctx),
            LootCondition::AnyOf { terms } => terms.iter().any(|c| c.check(ctx)),
            LootCondition::AllOf { terms } => terms.iter().all(|c| c.check(ctx)),
            LootCondition::Unknown => true,
        }
    }

    /// A tool predicate naming an enchantment the registry lacks is dropped, so the
    /// condition always holds.
    pub fn drop_unknown_enchantments(&mut self, registry: &StaticRegistry<EnchantmentData>) {
        match self {
            LootCondition::MatchTool { predicate } => {
                if let Some(required) = predicate.enchantment()
                    && registry.id_of(required.enchantments.as_str()).is_none()
                {
                    warn!(
                        enchantment = %required.enchantments,
                        "Enchantment not found in registry, condition will always be true"
                    );
                    predicate.predicates = None;
                }
            }
            LootCondition::Inverted { term } => term.drop_unknown_enchantments(registry),
            LootCondition::AnyOf { terms } | LootCondition::AllOf { terms } => {
                for term in terms {
                    term.drop_unknown_enchantments(registry);
                }
            }
            LootCondition::SurvivesExplosion {} | LootCondition::Unknown => {}
        }
    }
}
