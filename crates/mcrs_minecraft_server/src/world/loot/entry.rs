use crate::world::loot::condition::LootCondition;
use crate::world::loot::context::{BlockBreakContext, LootDrop};
use mcrs_minecraft_core::ResourceLocation;
use mcrs_minecraft_item::enchantment::EnchantmentData;
use mcrs_minecraft_registry::StaticRegistry;
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type")]
pub enum LootEntry {
    #[serde(rename = "minecraft:item")]
    Item {
        name: ResourceLocation,
        #[serde(default)]
        conditions: Vec<LootCondition>,
    },
    #[serde(rename = "minecraft:alternatives")]
    Alternatives {
        children: Vec<LootEntry>,
        #[serde(default)]
        conditions: Vec<LootCondition>,
    },
    #[serde(rename = "minecraft:empty")]
    Empty {
        #[serde(default)]
        conditions: Vec<LootCondition>,
    },
    #[serde(other)]
    Unknown,
}

impl LootEntry {
    pub fn evaluate(&self, ctx: &BlockBreakContext) -> Option<LootDrop> {
        match self {
            LootEntry::Item { name, conditions } => {
                conditions.iter().all(|c| c.check(ctx)).then(|| LootDrop {
                    item_name: name.clone(),
                    count: 1,
                })
            }
            LootEntry::Alternatives {
                children,
                conditions,
            } => {
                if !conditions.iter().all(|c| c.check(ctx)) {
                    return None;
                }
                children.iter().find_map(|child| child.evaluate(ctx))
            }
            LootEntry::Empty { .. } | LootEntry::Unknown => None,
        }
    }

    pub fn drop_unknown_enchantments(&mut self, registry: &StaticRegistry<EnchantmentData>) {
        let conditions = match self {
            LootEntry::Item { conditions, .. } | LootEntry::Empty { conditions } => conditions,
            LootEntry::Alternatives {
                children,
                conditions,
            } => {
                for child in children {
                    child.drop_unknown_enchantments(registry);
                }
                conditions
            }
            LootEntry::Unknown => return,
        };
        for condition in conditions {
            condition.drop_unknown_enchantments(registry);
        }
    }
}
