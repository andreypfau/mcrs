use crate::tag::block;
use crate::tag::block::{BlockTagSet, BlockTagSetExt};
use crate::world::block::Block;
use crate::world::item::component::ItemComponents;
use bevy_ecs::component::Component;

#[derive(Clone, Debug, Default, Component)]
pub struct Tool {
    pub rules: &'static [ToolRule],
    pub default_mining_speed: Option<f32>,
    pub damage_per_block: Option<u32>,
    pub can_destroy_blocks_in_creative: Option<bool>,
}

impl Tool {
    pub const fn new(
        rules: &'static [ToolRule],
        default_mining_speed: f32,
        damage_per_block: u32,
        can_destroy_blocks_in_creative: bool,
    ) -> Self {
        Self {
            rules,
            default_mining_speed: Some(default_mining_speed),
            damage_per_block: Some(damage_per_block),
            can_destroy_blocks_in_creative: Some(can_destroy_blocks_in_creative),
        }
    }

    pub fn get_mining_speed(&self, block: &Block) -> f32 {
        for rule in self.rules {
            let Some(speed) = rule.speed else {
                continue;
            };
            if rule.blocks.contains_block(block) {
                return speed;
            }
        }
        self.default_mining_speed()
    }

    pub fn is_correct_block_for_drops(&self, block: &Block) -> bool {
        for rule in self.rules {
            let Some(correct) = rule.correct_for_drops else {
                continue;
            };
            if rule.blocks.contains_block(block) {
                return correct;
            }
        }
        false
    }

    #[inline]
    pub fn rules(&self) -> &[ToolRule] {
        &self.rules
    }

    #[inline]
    pub fn default_mining_speed(&self) -> f32 {
        self.default_mining_speed.unwrap_or(1.0)
    }

    #[inline]
    pub fn damage_per_block(&self) -> u32 {
        self.damage_per_block.unwrap_or(1)
    }

    #[inline]
    pub fn can_destroy_blocks_in_creative(&self) -> bool {
        self.can_destroy_blocks_in_creative.unwrap_or(false)
    }
}

#[derive(Clone, Debug, Copy, Default)]
pub struct ToolRule {
    pub blocks: BlockTagSet,
    pub speed: Option<f32>,
    pub correct_for_drops: Option<bool>,
}

impl ToolRule {
    pub const fn mines_and_drops(blocks: BlockTagSet, speed: f32) -> Self {
        Self {
            blocks,
            speed: Some(speed),
            correct_for_drops: Some(true),
        }
    }

    pub const fn denies_drops(blocks: BlockTagSet) -> Self {
        Self {
            blocks,
            speed: None,
            correct_for_drops: Some(false),
        }
    }
}

pub struct ToolMaterial {
    incorrect_blocks_for_drops: BlockTagSet,
    durability: u32,
    speed: f32,
    attack_damage_bonus: f32,
    enchantment_value: u8,
}

impl ToolMaterial {
    pub const WOOD: ToolMaterial = ToolMaterial {
        incorrect_blocks_for_drops: block::INCORRECT_FOR_WOODEN_TOOL,
        durability: 59,
        speed: 2.0,
        attack_damage_bonus: 0.0,
        enchantment_value: 15,
    };
    pub const STONE: ToolMaterial = ToolMaterial {
        incorrect_blocks_for_drops: block::INCORRECT_FOR_STONE_TOOL,
        durability: 131,
        speed: 4.0,
        attack_damage_bonus: 1.0,
        enchantment_value: 5,
    };
    pub const COPPER: ToolMaterial = ToolMaterial {
        incorrect_blocks_for_drops: block::INCORRECT_FOR_COPPER_TOOL,
        durability: 190,
        speed: 5.0,
        attack_damage_bonus: 1.0,
        enchantment_value: 13,
    };
    pub const IRON: ToolMaterial = ToolMaterial {
        incorrect_blocks_for_drops: block::INCORRECT_FOR_IRON_TOOL,
        durability: 250,
        speed: 6.0,
        attack_damage_bonus: 2.0,
        enchantment_value: 14,
    };
    pub const DIAMOND: ToolMaterial = ToolMaterial {
        incorrect_blocks_for_drops: block::INCORRECT_FOR_DIAMOND_TOOL,
        durability: 1561,
        speed: 8.0,
        attack_damage_bonus: 3.0,
        enchantment_value: 10,
    };
    pub const GOLD: ToolMaterial = ToolMaterial {
        incorrect_blocks_for_drops: block::INCORRECT_FOR_GOLD_TOOL,
        durability: 32,
        speed: 12.0,
        attack_damage_bonus: 0.0,
        enchantment_value: 22,
    };
    pub const NETHERITE: ToolMaterial = ToolMaterial {
        incorrect_blocks_for_drops: block::INCORRECT_FOR_NETHERITE_TOOL,
        durability: 2031,
        speed: 9.0,
        attack_damage_bonus: 4.0,
        enchantment_value: 15,
    };

    pub const fn incorrect_blocks_for_drops(&self) -> BlockTagSet {
        self.incorrect_blocks_for_drops
    }

    pub const fn speed(&self) -> f32 {
        self.speed
    }

    pub const fn apply_common_properties(&self, components: ItemComponents) -> ItemComponents {
        components
            .with_durability(self.durability)
            .with_enchantable(self.enchantment_value)
    }

    pub const fn for_mineable_blocks(&self, mineable: BlockTagSet) -> [ToolRule; 2] {
        [
            ToolRule::denies_drops(self.incorrect_blocks_for_drops),
            ToolRule::mines_and_drops(mineable, self.speed),
        ]
    }

    pub const fn apply_tool_properties(
        &self,
        components: ItemComponents,
        attack_damage: f32,
        attack_speed: f32,
        disable_blocking_for_seconds: f32,
        rules: &'static [ToolRule],
    ) -> ItemComponents {
        self.apply_common_properties(components).with_tool(Tool {
            rules,
            default_mining_speed: Some(self.speed),
            damage_per_block: Some(1),
            can_destroy_blocks_in_creative: Some(true),
        })
    }
}
