use mcrs_minecraft_assets::tag::registry::DynTagRegistry;
use mcrs_minecraft_block::Block;
use mcrs_minecraft_block::definition::BlockDefinitions;
use mcrs_minecraft_block::tags as block_tags;
use mcrs_minecraft_core::codec::Bounded;
use mcrs_minecraft_core::tag_key::TagKey;
use mcrs_minecraft_core::{HolderSet, ResourceKey};
use mcrs_minecraft_protocol::item::{
    BlockReg, ComponentMap, Damage, Enchantable, MaxDamage, MaxStackSize, Tool, ToolRule,
};

pub struct ToolMaterial {
    incorrect_blocks_for_drops: TagKey<Block>,
    durability: i32,
    speed: f32,
    enchantment_value: i32,
}

impl ToolMaterial {
    pub const WOOD: ToolMaterial = ToolMaterial {
        incorrect_blocks_for_drops: block_tags::INCORRECT_FOR_WOODEN_TOOL,
        durability: 59,
        speed: 2.0,
        enchantment_value: 15,
    };
    pub const STONE: ToolMaterial = ToolMaterial {
        incorrect_blocks_for_drops: block_tags::INCORRECT_FOR_STONE_TOOL,
        durability: 131,
        speed: 4.0,
        enchantment_value: 5,
    };
    pub const COPPER: ToolMaterial = ToolMaterial {
        incorrect_blocks_for_drops: block_tags::INCORRECT_FOR_COPPER_TOOL,
        durability: 190,
        speed: 5.0,
        enchantment_value: 13,
    };
    pub const IRON: ToolMaterial = ToolMaterial {
        incorrect_blocks_for_drops: block_tags::INCORRECT_FOR_IRON_TOOL,
        durability: 250,
        speed: 6.0,
        enchantment_value: 14,
    };
    pub const DIAMOND: ToolMaterial = ToolMaterial {
        incorrect_blocks_for_drops: block_tags::INCORRECT_FOR_DIAMOND_TOOL,
        durability: 1561,
        speed: 8.0,
        enchantment_value: 10,
    };
    pub const GOLD: ToolMaterial = ToolMaterial {
        incorrect_blocks_for_drops: block_tags::INCORRECT_FOR_GOLD_TOOL,
        durability: 32,
        speed: 12.0,
        enchantment_value: 22,
    };
    pub const NETHERITE: ToolMaterial = ToolMaterial {
        incorrect_blocks_for_drops: block_tags::INCORRECT_FOR_NETHERITE_TOOL,
        durability: 2031,
        speed: 9.0,
        enchantment_value: 15,
    };

    pub fn tool(&self, mines_efficiently: TagKey<Block>) -> ComponentMap {
        let tag = |key: TagKey<Block>| HolderSet::Tag(key.resource_location_arc());
        ComponentMap(vec![
            MaxStackSize(Bounded(1)).into(),
            MaxDamage(Bounded(self.durability)).into(),
            Damage(Bounded(0)).into(),
            Enchantable {
                value: Bounded(self.enchantment_value),
            }
            .into(),
            Tool {
                rules: vec![
                    ToolRule {
                        blocks: tag(self.incorrect_blocks_for_drops),
                        speed: None,
                        correct_for_drops: Some(false),
                    },
                    ToolRule {
                        blocks: tag(mines_efficiently),
                        speed: Some(self.speed),
                        correct_for_drops: Some(true),
                    },
                ],
                ..Tool::default()
            }
            .into(),
        ])
    }
}

pub fn mining_speed(
    tool: &Tool,
    block: &str,
    blocks: &BlockDefinitions,
    tags: &DynTagRegistry<Block>,
) -> f32 {
    tool.rules
        .iter()
        .find(|rule| rule.speed.is_some() && contains(&rule.blocks, block, blocks, tags))
        .and_then(|rule| rule.speed)
        .unwrap_or(tool.default_mining_speed)
}

pub fn is_correct_for_drops(
    tool: &Tool,
    block: &str,
    blocks: &BlockDefinitions,
    tags: &DynTagRegistry<Block>,
) -> bool {
    tool.rules
        .iter()
        .find(|rule| {
            rule.correct_for_drops.is_some() && contains(&rule.blocks, block, blocks, tags)
        })
        .and_then(|rule| rule.correct_for_drops)
        .unwrap_or(false)
}

fn contains(
    set: &HolderSet<ResourceKey<BlockReg>>,
    block: &str,
    blocks: &BlockDefinitions,
    tags: &DynTagRegistry<Block>,
) -> bool {
    match set {
        HolderSet::Tag(tag) => blocks
            .index_of(block)
            .is_some_and(|id| tags.contains(&TagKey::<Block, _>::from_location(tag.clone()), id)),
        _ => set.entries().iter().any(|key| key.as_str() == block),
    }
}
