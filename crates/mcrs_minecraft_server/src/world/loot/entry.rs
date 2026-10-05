use crate::world::loot::Unapplied;
use crate::world::loot::condition::{Condition, holds};
use crate::world::loot::context::{BlockBreakContext, LootDrop};
use mcrs_minecraft_core::ResourceLocation;
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", deny_unknown_fields)]
pub enum LootEntry {
    #[serde(rename = "minecraft:item")]
    Item {
        name: ResourceLocation,
        condition: Option<Condition>,
        #[serde(default, rename = "modifier")]
        _modifier: Unapplied,
    },
    #[serde(rename = "minecraft:alternatives")]
    Alternatives {
        #[serde(default)]
        children: Vec<LootEntry>,
        condition: Option<Condition>,
        #[serde(default, rename = "modifier")]
        _modifier: Unapplied,
    },
    #[serde(rename = "minecraft:empty")]
    Empty {
        condition: Option<Condition>,
        #[serde(default, rename = "modifier")]
        _modifier: Unapplied,
    },
    #[serde(other)]
    Unknown,
}

impl LootEntry {
    pub fn evaluate(&self, ctx: &BlockBreakContext) -> Option<LootDrop> {
        match self {
            LootEntry::Item {
                name, condition, ..
            } => holds(condition, ctx).then(|| LootDrop {
                item_name: name.clone(),
                count: 1,
            }),
            LootEntry::Alternatives {
                children,
                condition,
                ..
            } => {
                if !holds(condition, ctx) {
                    return None;
                }
                children.iter().find_map(|child| child.evaluate(ctx))
            }
            LootEntry::Empty { .. } | LootEntry::Unknown => None,
        }
    }

    pub(crate) fn conditions_mut(&mut self, visit: &mut impl FnMut(&mut Condition)) {
        let condition = match self {
            LootEntry::Item { condition, .. } | LootEntry::Empty { condition, .. } => condition,
            LootEntry::Alternatives {
                children,
                condition,
                ..
            } => {
                for child in children {
                    child.conditions_mut(visit);
                }
                condition
            }
            LootEntry::Unknown => return,
        };
        if let Some(condition) = condition {
            visit(condition);
        }
    }
}
