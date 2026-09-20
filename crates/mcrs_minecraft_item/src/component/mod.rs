use mcrs_minecraft_assets::tag::registry::DynTagRegistry;
use mcrs_minecraft_block::Block;
use mcrs_minecraft_block::definition::BlockDefinitions;
use mcrs_minecraft_block::tags as block_tags;
use mcrs_minecraft_core::codec::Bounded;
use mcrs_minecraft_core::resource_location::ResourceLocation;
use mcrs_minecraft_core::tag_key::TagKey;
use mcrs_minecraft_core::{HolderSet, ResourceKey};
use mcrs_minecraft_protocol::item::{
    AttackAnimation, AttributeModifiers, BlockReg, BreakSound, ComponentMap, Damage, Enchantable,
    Enchantments, Holder, InteractAnimation, Lore, MaxDamage, MaxStackSize, Rarity, RepairCost,
    SwingAnimation, Tool, ToolRule, TooltipDisplay, UseEffects,
};

pub fn common_item_components() -> ComponentMap {
    ComponentMap(vec![
        MaxStackSize::default().into(),
        Lore::default().into(),
        Enchantments::default().into(),
        RepairCost(Bounded(0)).into(),
        UseEffects::default().into(),
        AttributeModifiers::default().into(),
        Rarity::Common.into(),
        BreakSound(Holder::reference(ResourceLocation::minecraft(
            "entity.item.break",
        )))
        .into(),
        TooltipDisplay::default().into(),
        AttackAnimation(SwingAnimation::default()).into(),
        InteractAnimation(SwingAnimation::default()).into(),
    ])
}

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
        let mut map = common_item_components();
        map.set_value(MaxStackSize(Bounded(1)).into());
        map.set_value(MaxDamage(Bounded(self.durability)).into());
        map.set_value(Damage(Bounded(0)).into());
        map.set_value(
            Enchantable {
                value: Bounded(self.enchantment_value),
            }
            .into(),
        );
        map.set_value(
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
        );
        map
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

#[cfg(test)]
mod tests {
    use super::*;
    use mcrs_minecraft_protocol::item::{ComponentPatch, SwingAnimationKind};

    #[test]
    fn common_item_components_match_vanilla() {
        let map = common_item_components();
        assert_eq!(map.0.len(), 11);
        assert_eq!(map.get::<MaxStackSize>(), Some(&MaxStackSize(Bounded(64))));
        assert_eq!(map.get::<Lore>().unwrap().lines(), &Vec::new());
        assert_eq!(map.get::<Enchantments>(), Some(&Enchantments(vec![])));
        assert_eq!(map.get::<RepairCost>(), Some(&RepairCost(Bounded(0))));
        assert_eq!(
            map.get::<UseEffects>(),
            Some(&UseEffects {
                can_sprint: false,
                interact_vibrations: true,
                speed_multiplier: 0.2,
            })
        );
        assert_eq!(
            map.get::<AttributeModifiers>(),
            Some(&AttributeModifiers(vec![]))
        );
        assert_eq!(map.get::<Rarity>(), Some(&Rarity::Common));
        assert_eq!(
            map.get::<BreakSound>(),
            Some(&BreakSound(Holder::reference(ResourceLocation::minecraft(
                "entity.item.break"
            ))))
        );
        assert_eq!(
            map.get::<TooltipDisplay>(),
            Some(&TooltipDisplay::new(false, vec![]))
        );
        let whack = SwingAnimation {
            kind: SwingAnimationKind::Whack,
            duration: Bounded(6),
        };
        assert_eq!(map.get::<AttackAnimation>(), Some(&AttackAnimation(whack)));
        assert_eq!(
            map.get::<InteractAnimation>(),
            Some(&InteractAnimation(whack))
        );
        assert_eq!(map.diff(&map), ComponentPatch::EMPTY);
    }
}
