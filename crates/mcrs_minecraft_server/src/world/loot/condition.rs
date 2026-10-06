use crate::world::loot::context::BlockBreakContext;
use mcrs_minecraft_block::definition::BlockEntry;
use mcrs_minecraft_block::keys::Block;
use mcrs_minecraft_core::ResourceLocation;
use mcrs_minecraft_item::enchantment::EnchantmentData;
use mcrs_minecraft_item::keys::Item;
use mcrs_minecraft_registry::{HolderSet, Registry};
use rustc_hash::FxHashMap;
use serde::de::{IgnoredAny, MapAccess, Visitor, value};
use serde::{Deserialize, Deserializer};
use std::fmt;
use tracing::warn;

// chisle: the block-break context has no random source, position, tool item or
// tags, so a condition that needs one does not hold. That is the outcome vanilla
// gives most often for a player breaking by hand: no shears, no lucky roll. The
// richer loot context (#70) and typed conditions (#73) lift this.
const UNDECIDABLE: bool = false;

/// A predicate named from the `minecraft:predicate` registry, or a condition
/// written in place. The loader inlines every name, so a loaded table holds
/// `Inline` alone.
#[derive(Debug, Clone)]
pub enum Condition {
    Named(ResourceLocation),
    Inline(Box<LootCondition>),
}

impl<'de> Deserialize<'de> for Condition {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct ConditionVisitor;

        impl<'de> Visitor<'de> for ConditionVisitor {
            type Value = Condition;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a predicate id or an inline condition")
            }

            fn visit_str<E: serde::de::Error>(self, id: &str) -> Result<Self::Value, E> {
                ResourceLocation::read(id)
                    .map(Condition::Named)
                    .map_err(E::custom)
            }

            fn visit_map<A: MapAccess<'de>>(self, map: A) -> Result<Self::Value, A::Error> {
                LootCondition::deserialize(value::MapAccessDeserializer::new(map))
                    .map(|condition| Condition::Inline(Box::new(condition)))
            }
        }

        deserializer.deserialize_any(ConditionVisitor)
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", deny_unknown_fields)]
pub enum LootCondition {
    #[serde(rename = "minecraft:inverted")]
    Inverted { term: Condition },
    #[serde(rename = "minecraft:any_of")]
    AnyOf { terms: Vec<Condition> },
    #[serde(rename = "minecraft:all_of")]
    AllOf { terms: Vec<Condition> },
    #[serde(rename = "minecraft:match_tool")]
    MatchTool { predicate: ToolPredicate },
    #[serde(rename = "minecraft:match_block")]
    MatchBlock {
        blocks: Option<HolderSet<Block>>,
        state: Option<StatePredicate>,
    },
    #[serde(rename = "minecraft:survives_explosion")]
    SurvivesExplosion {},
    #[serde(rename = "minecraft:entity_properties")]
    EntityProperties {
        entity: EntityTarget,
        predicate: Option<EntityPredicate>,
    },
    // chisle: read without their fields and decided by `UNDECIDABLE`; typed
    // conditions (#73) lift this.
    #[serde(
        rename = "minecraft:random_chance",
        alias = "minecraft:random_chance_with_enchanted_bonus",
        alias = "minecraft:table_bonus",
        alias = "minecraft:location_check",
        alias = "minecraft:killed_by_player",
        alias = "minecraft:entity_scores",
        alias = "minecraft:damage_source_properties",
        alias = "minecraft:weather_check",
        alias = "minecraft:time_check",
        alias = "minecraft:int_value_check",
        alias = "minecraft:float_value_check",
        alias = "minecraft:enchantment_active_check",
        alias = "minecraft:environment_attribute_check"
    )]
    Undecidable(IgnoredAny),
}

const LOOT_CONDITION_TYPE_ROWS: &[&str] = &[
    "minecraft:inverted",
    "minecraft:any_of",
    "minecraft:all_of",
    "minecraft:match_tool",
    "minecraft:match_block",
    "minecraft:survives_explosion",
    "minecraft:entity_properties",
    "minecraft:random_chance",
    "minecraft:random_chance_with_enchanted_bonus",
    "minecraft:table_bonus",
    "minecraft:location_check",
    "minecraft:killed_by_player",
    "minecraft:entity_scores",
    "minecraft:damage_source_properties",
    "minecraft:weather_check",
    "minecraft:time_check",
    "minecraft:int_value_check",
    "minecraft:float_value_check",
    "minecraft:enchantment_active_check",
    "minecraft:environment_attribute_check",
];

const _: () = assert!(mcrs_minecraft_registry::static_rows::names_cover(
    LOOT_CONDITION_TYPE_ROWS,
    &[],
    mcrs_minecraft_enchantment::keys::LootConditionType::ENTRIES
));

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ToolPredicate {
    pub items: Option<HolderSet<Item>>,
    pub predicates: Option<ToolPredicates>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ToolPredicates {
    #[serde(rename = "minecraft:enchantments")]
    pub enchantments: Option<Vec<EnchantmentPredicate>>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EnchantmentPredicate {
    pub enchantments: ResourceLocation,
    pub levels: Option<LevelRange>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LevelRange {
    pub min: Option<u8>,
}

/// Property name to the exact value it must render as.
#[derive(Debug, Clone, Deserialize)]
pub struct StatePredicate(FxHashMap<Box<str>, Box<str>>);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EntityTarget {
    This,
    Attacker,
    DirectAttacker,
    AttackingPlayer,
    TargetEntity,
    InteractingEntity,
}

/// Only the empty predicate, which every present entity matches.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EntityPredicate {}

pub fn holds(condition: &Option<Condition>, ctx: &BlockBreakContext) -> bool {
    condition.as_ref().is_none_or(|c| c.check(ctx))
}

impl ToolPredicate {
    fn enchantments(&self) -> &[EnchantmentPredicate] {
        self.predicates
            .as_ref()
            .and_then(|p| p.enchantments.as_deref())
            .unwrap_or_default()
    }

    fn matches(&self, ctx: &BlockBreakContext) -> bool {
        let items_match = self.items.is_none() || UNDECIDABLE;
        items_match
            && self.enchantments().iter().all(|required| {
                let min_level = required.levels.as_ref().and_then(|l| l.min).unwrap_or(1);
                ctx.tool_enchantments.is_some_and(|enchantments| {
                    enchantments.level(&required.enchantments) >= i32::from(min_level)
                })
            })
    }
}

impl StatePredicate {
    fn matches(&self, block: &BlockEntry, ctx: &BlockBreakContext) -> bool {
        self.0.iter().all(|(property, expected)| {
            block
                .value_of(ctx.state, property)
                .is_some_and(|value| value.renders_to(expected))
        })
    }
}

impl Condition {
    pub fn check(&self, ctx: &BlockBreakContext) -> bool {
        match self {
            Condition::Inline(condition) => condition.check(ctx),
            Condition::Named(name) => unreachable!("the loader inlines `{name}`"),
        }
    }

    pub(crate) fn names(&self, into: &mut Vec<ResourceLocation>) {
        match self {
            Condition::Named(name) => into.push(name.clone()),
            Condition::Inline(condition) => condition.terms().for_each(|t| t.names(into)),
        }
    }

    /// Replaces every name with its predicate, refusing a predicate that
    /// reaches itself.
    pub(crate) fn inline(
        &mut self,
        predicates: &FxHashMap<ResourceLocation, LootCondition>,
        stack: &mut Vec<ResourceLocation>,
    ) -> Result<(), String> {
        if let Condition::Named(name) = self {
            if stack.contains(name) {
                return Err(format!("predicate `{name}` refers to itself"));
            }
            let predicate = predicates
                .get(name)
                .ok_or_else(|| format!("predicate `{name}` was not read"))?;
            let name = name.clone();
            *self = Condition::Inline(Box::new(predicate.clone()));
            stack.push(name);
            let inlined = self.inline(predicates, stack);
            stack.pop();
            return inlined;
        }
        let Condition::Inline(condition) = self else {
            unreachable!()
        };
        condition
            .terms_mut()
            .try_for_each(|term| term.inline(predicates, stack))
    }

    /// A tool predicate naming an enchantment the registry lacks is dropped, so
    /// the condition always holds.
    pub fn drop_unknown_enchantments(&mut self, registry: &Registry<EnchantmentData>) {
        let Condition::Inline(condition) = self else {
            return;
        };
        if let LootCondition::MatchTool { predicate } = &mut **condition
            && let Some(unknown) = predicate
                .enchantments()
                .iter()
                .find(|required| registry.by_name(required.enchantments.as_str()).is_none())
        {
            warn!(
                enchantment = %unknown.enchantments,
                "Enchantment not found in registry, condition will always be true"
            );
            predicate.predicates = None;
        }
        for term in condition.terms_mut() {
            term.drop_unknown_enchantments(registry);
        }
    }
}

impl LootCondition {
    pub fn check(&self, ctx: &BlockBreakContext) -> bool {
        match self {
            LootCondition::Inverted { term } => !term.check(ctx),
            LootCondition::AnyOf { terms } => terms.iter().any(|c| c.check(ctx)),
            LootCondition::AllOf { terms } => terms.iter().all(|c| c.check(ctx)),
            LootCondition::MatchTool { predicate } => predicate.matches(ctx),
            LootCondition::MatchBlock { blocks, state } => {
                let block = ctx.blocks.owner(ctx.state);
                let block_matches = blocks
                    .as_ref()
                    .is_none_or(|set| set.contains(ctx.blocks.block_index(ctx.state), ctx.tags));
                block_matches && state.as_ref().is_none_or(|s| s.matches(block, ctx))
            }
            LootCondition::SurvivesExplosion {} => true,
            // A block-break context is a player breaking the block: `this` is
            // that player, and no other entity is present.
            LootCondition::EntityProperties { entity, predicate } => {
                predicate.is_none() || *entity == EntityTarget::This
            }
            LootCondition::Undecidable(_) => UNDECIDABLE,
        }
    }

    pub(crate) fn terms(&self) -> impl Iterator<Item = &Condition> {
        let terms: &[Condition] = match self {
            LootCondition::Inverted { term } => std::slice::from_ref(term),
            LootCondition::AnyOf { terms } | LootCondition::AllOf { terms } => terms,
            _ => &[],
        };
        terms.iter()
    }

    fn terms_mut(&mut self) -> impl Iterator<Item = &mut Condition> {
        let terms: &mut [Condition] = match self {
            LootCondition::Inverted { term } => std::slice::from_mut(term),
            LootCondition::AnyOf { terms } | LootCondition::AllOf { terms } => terms,
            _ => &mut [],
        };
        terms.iter_mut()
    }
}

#[cfg(test)]
mod dispatch_rows {
    use super::*;

    #[test]
    fn loot_condition_type_rows_select_their_variants() {
        mcrs_minecraft_registry::static_rows::assert_dispatch::<LootCondition>(
            LOOT_CONDITION_TYPE_ROWS,
            &[],
            mcrs_minecraft_enchantment::keys::LootConditionType::ENTRIES,
            |name| serde_json::json!({ "type": name }),
        );
    }
}
