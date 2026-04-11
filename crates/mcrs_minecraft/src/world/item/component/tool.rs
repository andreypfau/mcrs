use crate::tag::block::{BlockTagSet, BlockTagSetExt};
use crate::world::block::Block;
use crate::world::item::component::ItemComponents;
use bevy_ecs::component::Component;
use mcrs_core::tag::key::TagKey;
use mcrs_core::tag::registry::TagRegistry;
use mcrs_core::{ResourceLocation, StaticRegistry};
use mcrs_vanilla::block::Block as VanillaBlock;

#[derive(Clone, Copy, Debug)]
pub enum ToolTagRef {
    Static(BlockTagSet),
    DynamicIdent(&'static str),
}

impl Default for ToolTagRef {
    fn default() -> Self {
        ToolTagRef::Static(&[])
    }
}

impl ToolTagRef {
    pub const fn from_static(blocks: BlockTagSet) -> Self {
        ToolTagRef::Static(blocks)
    }

    pub const fn from_dynamic_ident(ident: &'static str) -> Self {
        ToolTagRef::DynamicIdent(ident)
    }

    pub fn contains_block(&self, block: &Block) -> bool {
        match self {
            ToolTagRef::Static(tag_set) => tag_set.contains_block(block),
            ToolTagRef::DynamicIdent(_) => false,
        }
    }

    pub fn contains_block_with_registry(
        &self,
        block: &Block,
        tag_registry: &TagRegistry<VanillaBlock>,
        block_registry: &StaticRegistry<VanillaBlock>,
    ) -> bool {
        match self {
            ToolTagRef::Static(tag_set) => tag_set.contains_block(block),
            ToolTagRef::DynamicIdent(ident_str) => {
                let tag_key = TagKey::<VanillaBlock>::new(
                    ResourceLocation::new_static(ident_str),
                );
                let Some(static_id) = block_registry.id_of(block.identifier.as_ref()) else {
                    return false;
                };
                tag_registry.contains(&tag_key, static_id)
            }
        }
    }

    pub const fn is_static(&self) -> bool {
        matches!(self, ToolTagRef::Static(_))
    }

    pub const fn is_dynamic(&self) -> bool {
        matches!(self, ToolTagRef::DynamicIdent(_))
    }

    pub const fn as_dynamic_ident(&self) -> Option<&'static str> {
        match self {
            ToolTagRef::DynamicIdent(ident) => Some(*ident),
            ToolTagRef::Static(_) => None,
        }
    }
}

impl From<BlockTagSet> for ToolTagRef {
    fn from(blocks: BlockTagSet) -> Self {
        ToolTagRef::Static(blocks)
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
        tag_registry: &TagRegistry<VanillaBlock>,
        block_registry: &StaticRegistry<VanillaBlock>,
    ) -> f32 {
        for rule in self.rules {
            let Some(speed) = rule.speed else {
                continue;
            };
            if rule.blocks.contains_block_with_registry(block, tag_registry, block_registry) {
                return speed;
            }
        }
        self.default_mining_speed()
    }

    pub fn is_correct_block_for_drops(
        &self,
        block: &Block,
        tag_registry: &TagRegistry<VanillaBlock>,
        block_registry: &StaticRegistry<VanillaBlock>,
    ) -> bool {
        for (i, rule) in self.rules.iter().enumerate() {
            let Some(correct) = rule.correct_for_drops else {
                continue;
            };
            let matched = rule.blocks.contains_block_with_registry(block, tag_registry, block_registry);
            tracing::debug!(
                rule_index = i,
                block = %block.identifier,
                tag = ?rule.blocks.as_dynamic_ident(),
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

#[derive(Clone, Copy, Debug, Default)]
pub struct ToolRule {
    pub blocks: ToolTagRef,
    pub speed: Option<f32>,
    pub correct_for_drops: Option<bool>,
}

impl ToolRule {
    pub const fn mines_and_drops(blocks: BlockTagSet, speed: f32) -> Self {
        Self {
            blocks: ToolTagRef::Static(blocks),
            speed: Some(speed),
            correct_for_drops: Some(true),
        }
    }

    pub const fn denies_drops(blocks: BlockTagSet) -> Self {
        Self {
            blocks: ToolTagRef::Static(blocks),
            speed: None,
            correct_for_drops: Some(false),
        }
    }

    pub const fn mines_and_drops_dynamic(tag_ident: &'static str, speed: f32) -> Self {
        Self {
            blocks: ToolTagRef::DynamicIdent(tag_ident),
            speed: Some(speed),
            correct_for_drops: Some(true),
        }
    }

    pub const fn denies_drops_dynamic(tag_ident: &'static str) -> Self {
        Self {
            blocks: ToolTagRef::DynamicIdent(tag_ident),
            speed: None,
            correct_for_drops: Some(false),
        }
    }

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
        incorrect_blocks_for_drops: ToolTagRef::DynamicIdent("minecraft:incorrect_for_wooden_tool"),
        durability: 59,
        speed: 2.0,
        attack_damage_bonus: 0.0,
        enchantment_value: 15,
    };
    pub const STONE: ToolMaterial = ToolMaterial {
        incorrect_blocks_for_drops: ToolTagRef::DynamicIdent("minecraft:incorrect_for_stone_tool"),
        durability: 131,
        speed: 4.0,
        attack_damage_bonus: 1.0,
        enchantment_value: 5,
    };
    pub const COPPER: ToolMaterial = ToolMaterial {
        incorrect_blocks_for_drops: ToolTagRef::DynamicIdent("minecraft:incorrect_for_copper_tool"),
        durability: 190,
        speed: 5.0,
        attack_damage_bonus: 1.0,
        enchantment_value: 13,
    };
    pub const IRON: ToolMaterial = ToolMaterial {
        incorrect_blocks_for_drops: ToolTagRef::DynamicIdent("minecraft:incorrect_for_iron_tool"),
        durability: 250,
        speed: 6.0,
        attack_damage_bonus: 2.0,
        enchantment_value: 14,
    };
    pub const DIAMOND: ToolMaterial = ToolMaterial {
        incorrect_blocks_for_drops: ToolTagRef::DynamicIdent("minecraft:incorrect_for_diamond_tool"),
        durability: 1561,
        speed: 8.0,
        attack_damage_bonus: 3.0,
        enchantment_value: 10,
    };
    pub const GOLD: ToolMaterial = ToolMaterial {
        incorrect_blocks_for_drops: ToolTagRef::DynamicIdent("minecraft:incorrect_for_gold_tool"),
        durability: 32,
        speed: 12.0,
        attack_damage_bonus: 0.0,
        enchantment_value: 22,
    };
    pub const NETHERITE: ToolMaterial = ToolMaterial {
        incorrect_blocks_for_drops: ToolTagRef::DynamicIdent("minecraft:incorrect_for_netherite_tool"),
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

    pub const fn for_mineable_blocks(&self, mineable: BlockTagSet) -> [ToolRule; 2] {
        [
            ToolRule::with_tag_ref(self.incorrect_blocks_for_drops, None, Some(false)),
            ToolRule::mines_and_drops(mineable, self.speed),
        ]
    }

    pub const fn for_mineable_blocks_dynamic(&self, mineable_tag: &'static str) -> [ToolRule; 2] {
        [
            ToolRule::with_tag_ref(self.incorrect_blocks_for_drops, None, Some(false)),
            ToolRule::mines_and_drops_dynamic(mineable_tag, self.speed),
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
