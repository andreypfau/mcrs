use crate::block::tags as block_tags;
use crate::block::Block;
use crate::item::component::ItemComponents;
use bevy_ecs::component::Component;
use mcrs_core::tag::key::TagKey;
use mcrs_core::{StaticId, StaticRegistry, StaticTags};

/// Reference to a set of blocks via a tag key.
///
/// All tool tag references are dynamic (loaded from asset files at runtime via
/// `StaticTags<Block>`). The static compile-time `BlockTagSet` approach has been
/// removed in favour of this unified `TagKey`-based approach.
#[derive(Clone, Copy, Debug)]
pub struct ToolTagRef {
    pub tag_key: TagKey<Block>,
}

impl Default for ToolTagRef {
    fn default() -> Self {
        ToolTagRef {
            tag_key: block_tags::MINEABLE_PICKAXE,
        }
    }
}

impl ToolTagRef {
    pub const fn new(tag_key: TagKey<Block>) -> Self {
        Self { tag_key }
    }

    /// Checks if the given block (identified by its `StaticId`) is in this tag.
    pub fn contains_block(
        &self,
        block_id: StaticId<Block>,
        static_tags: &StaticTags<Block>,
    ) -> bool {
        static_tags.contains(&self.tag_key, block_id)
    }
}

impl From<TagKey<Block>> for ToolTagRef {
    fn from(tag_key: TagKey<Block>) -> Self {
        Self { tag_key }
    }
}

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

    pub fn get_mining_speed(
        &self,
        block: &Block,
        block_registry: &StaticRegistry<Block>,
        static_tags: &StaticTags<Block>,
    ) -> f32 {
        let block_id = block_registry.id_of(block.identifier.as_str());
        for rule in self.rules {
            let Some(speed) = rule.speed else {
                continue;
            };
            if let Some(id) = block_id {
                if rule.blocks.contains_block(id, static_tags) {
                    return speed;
                }
            }
        }
        self.default_mining_speed()
    }

    pub fn is_correct_block_for_drops(
        &self,
        block: &Block,
        block_registry: &StaticRegistry<Block>,
        static_tags: &StaticTags<Block>,
    ) -> bool {
        let block_id = block_registry.id_of(block.identifier.as_str());
        for (i, rule) in self.rules.iter().enumerate() {
            let Some(correct) = rule.correct_for_drops else {
                continue;
            };
            let matched = if let Some(id) = block_id {
                rule.blocks.contains_block(id, static_tags)
            } else {
                false
            };
            tracing::debug!(
                rule_index = i,
                block = %block.identifier,
                correct,
                matched,
                "is_correct_block_for_drops rule check"
            );
            if matched {
                return correct;
            }
        }
        false
    }

    #[inline]
    pub fn rules(&self) -> &[ToolRule] {
        self.rules
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

#[derive(Clone, Copy, Debug)]
pub struct ToolRule {
    pub blocks: ToolTagRef,
    pub speed: Option<f32>,
    pub correct_for_drops: Option<bool>,
}

impl Default for ToolRule {
    fn default() -> Self {
        Self {
            blocks: ToolTagRef::default(),
            speed: None,
            correct_for_drops: None,
        }
    }
}

impl ToolRule {
    /// Creates a tool rule for blocks that can be mined at the given speed and drop items.
    pub const fn mines_and_drops(tag_key: TagKey<Block>, speed: f32) -> Self {
        Self {
            blocks: ToolTagRef { tag_key },
            speed: Some(speed),
            correct_for_drops: Some(true),
        }
    }

    /// Creates a tool rule for blocks that deny drops when mined with this tool.
    pub const fn denies_drops(tag_key: TagKey<Block>) -> Self {
        Self {
            blocks: ToolTagRef { tag_key },
            speed: None,
            correct_for_drops: Some(false),
        }
    }

    /// Creates a tool rule with a custom ToolTagRef.
    pub const fn with_tag_ref(
        blocks: ToolTagRef,
        speed: Option<f32>,
        correct_for_drops: Option<bool>,
    ) -> Self {
        Self {
            blocks,
            speed,
            correct_for_drops,
        }
    }
}

pub struct ToolMaterial {
    incorrect_blocks_for_drops: ToolTagRef,
    durability: u32,
    speed: f32,
    attack_damage_bonus: f32,
    enchantment_value: u8,
}

impl ToolMaterial {
    pub const WOOD: ToolMaterial = ToolMaterial {
        incorrect_blocks_for_drops: ToolTagRef {
            tag_key: TagKey::new(mcrs_core::rl!("minecraft:incorrect_for_wooden_tool")),
        },
        durability: 59,
        speed: 2.0,
        attack_damage_bonus: 0.0,
        enchantment_value: 15,
    };
    pub const STONE: ToolMaterial = ToolMaterial {
        incorrect_blocks_for_drops: ToolTagRef {
            tag_key: TagKey::new(mcrs_core::rl!("minecraft:incorrect_for_stone_tool")),
        },
        durability: 131,
        speed: 4.0,
        attack_damage_bonus: 1.0,
        enchantment_value: 5,
    };
    pub const COPPER: ToolMaterial = ToolMaterial {
        incorrect_blocks_for_drops: ToolTagRef {
            tag_key: TagKey::new(mcrs_core::rl!("minecraft:incorrect_for_copper_tool")),
        },
        durability: 190,
        speed: 5.0,
        attack_damage_bonus: 1.0,
        enchantment_value: 13,
    };
    pub const IRON: ToolMaterial = ToolMaterial {
        incorrect_blocks_for_drops: ToolTagRef {
            tag_key: TagKey::new(mcrs_core::rl!("minecraft:incorrect_for_iron_tool")),
        },
        durability: 250,
        speed: 6.0,
        attack_damage_bonus: 2.0,
        enchantment_value: 14,
    };
    pub const DIAMOND: ToolMaterial = ToolMaterial {
        incorrect_blocks_for_drops: ToolTagRef {
            tag_key: TagKey::new(mcrs_core::rl!("minecraft:incorrect_for_diamond_tool")),
        },
        durability: 1561,
        speed: 8.0,
        attack_damage_bonus: 3.0,
        enchantment_value: 10,
    };
    pub const GOLD: ToolMaterial = ToolMaterial {
        incorrect_blocks_for_drops: ToolTagRef {
            tag_key: TagKey::new(mcrs_core::rl!("minecraft:incorrect_for_gold_tool")),
        },
        durability: 32,
        speed: 12.0,
        attack_damage_bonus: 0.0,
        enchantment_value: 22,
    };
    pub const NETHERITE: ToolMaterial = ToolMaterial {
        incorrect_blocks_for_drops: ToolTagRef {
            tag_key: TagKey::new(mcrs_core::rl!("minecraft:incorrect_for_netherite_tool")),
        },
        durability: 2031,
        speed: 9.0,
        attack_damage_bonus: 4.0,
        enchantment_value: 15,
    };

    pub const fn incorrect_blocks_for_drops(&self) -> ToolTagRef {
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

    pub const fn for_mineable_blocks(&self, mineable: TagKey<Block>) -> [ToolRule; 2] {
        [
            ToolRule::with_tag_ref(self.incorrect_blocks_for_drops, None, Some(false)),
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
