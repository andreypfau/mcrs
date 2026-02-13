use crate::tag::block::{BlockTagSet, BlockTagSetExt, DynamicBlockTagSet, TagRegistry};
use crate::world::block::Block;
use crate::world::item::component::ItemComponents;
use bevy_ecs::component::Component;
use mcrs_protocol::Ident;
use std::str::FromStr;

/// Reference to a set of blocks, either static (compile-time) or dynamic (runtime lookup).
///
/// This enum allows the tool system to work with both hardcoded block tag constants
/// and dynamically loaded tags from asset files. The enum is `Copy` to support
/// use in const contexts.
///
/// # Examples
///
/// ```ignore
/// // Static reference (for const contexts)
/// let static_ref = ToolTagRef::Static(&[&BlockTag::Tag(&minecraft::STONE)]);
///
/// // Dynamic reference using a static identifier string (for const contexts)
/// let dynamic_ref = ToolTagRef::DynamicIdent("minecraft:mineable/pickaxe");
/// ```
#[derive(Clone, Copy, Debug)]
pub enum ToolTagRef {
    /// A static block tag set defined at compile time.
    Static(BlockTagSet),
    /// A dynamic tag identifier that will be resolved at runtime against the TagRegistry.
    /// The string should be in the format "namespace:path" (e.g., "minecraft:mineable/pickaxe").
    DynamicIdent(&'static str),
}

impl Default for ToolTagRef {
    fn default() -> Self {
        ToolTagRef::Static(&[])
    }
}

impl ToolTagRef {
    /// Creates a static tool tag reference from a compile-time block tag set.
    pub const fn from_static(blocks: BlockTagSet) -> Self {
        ToolTagRef::Static(blocks)
    }

    /// Creates a dynamic tool tag reference from a static identifier string.
    ///
    /// The identifier should be in the format "namespace:path" (e.g., "minecraft:mineable/pickaxe").
    pub const fn from_dynamic_ident(ident: &'static str) -> Self {
        ToolTagRef::DynamicIdent(ident)
    }

    /// Creates a dynamic tool tag reference from a DynamicBlockTagSet at runtime.
    ///
    /// Note: This consumes the DynamicBlockTagSet but only stores a reference to
    /// a leaked string. This should be used sparingly, preferring `from_dynamic_ident`
    /// for static identifier strings.
    pub fn from_dynamic(tag: DynamicBlockTagSet) -> Self {
        // For runtime creation, we need to leak the string to get a 'static reference.
        // This is intentional for cases where dynamic tags are created at runtime.
        let leaked = Box::leak(tag.ident.into_inner().into_boxed_str());
        ToolTagRef::DynamicIdent(leaked)
    }

    /// Checks if the given block is contained in this tag reference.
    ///
    /// For static tags, this performs a direct comparison.
    /// For dynamic tags, this requires the TagRegistry to perform the lookup.
    ///
    /// Note: For dynamic tags without a registry reference, this returns `false`.
    /// Use `contains_block_with_registry` for dynamic tag lookups.
    pub fn contains_block(&self, block: &Block) -> bool {
        match self {
            ToolTagRef::Static(tag_set) => tag_set.contains_block(block),
            ToolTagRef::DynamicIdent(_) => {
                // Dynamic tags require registry lookup; without registry, return false
                // Use contains_block_with_registry for proper dynamic lookup
                false
            }
        }
    }

    /// Checks if the given block is contained in this tag reference using the tag registry.
    ///
    /// This method supports both static and dynamic tag references:
    /// - Static tags perform direct comparison (registry is ignored)
    /// - Dynamic tags perform lookup against the provided TagRegistry
    pub fn contains_block_with_registry(
        &self,
        block: &Block,
        tag_registry: &TagRegistry<&'static Block>,
        block_registry: &mcrs_registry::Registry<&'static Block>,
    ) -> bool {
        match self {
            ToolTagRef::Static(tag_set) => tag_set.contains_block(block),
            ToolTagRef::DynamicIdent(ident_str) => {
                // Parse the identifier string and look up in registry
                if let Ok(ident) = Ident::<String>::from_str(ident_str) {
                    let dynamic_tag = DynamicBlockTagSet::new(ident);
                    // Find the block's registry ID
                    let reg_id = mcrs_registry::RegistryId::Identifier {
                        identifier: block.identifier.to_string_ident(),
                    };
                    if let Some((index, _)) = block_registry.get_full(reg_id) {
                        dynamic_tag.contains_block_index(tag_registry, index)
                    } else {
                        false
                    }
                } else {
                    false
                }
            }
        }
    }

    /// Returns true if this is a static tag reference.
    pub const fn is_static(&self) -> bool {
        matches!(self, ToolTagRef::Static(_))
    }

    /// Returns true if this is a dynamic tag reference.
    pub const fn is_dynamic(&self) -> bool {
        matches!(self, ToolTagRef::DynamicIdent(_))
    }

    /// Returns the dynamic identifier string if this is a dynamic tag reference.
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

impl From<DynamicBlockTagSet> for ToolTagRef {
    fn from(tag: DynamicBlockTagSet) -> Self {
        ToolTagRef::from_dynamic(tag)
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

#[derive(Clone, Copy, Debug, Default)]
pub struct ToolRule {
    pub blocks: ToolTagRef,
    pub speed: Option<f32>,
    pub correct_for_drops: Option<bool>,
}

impl ToolRule {
    /// Creates a tool rule for blocks that can be mined at the given speed and drop items.
    ///
    /// This is a const function that works with static BlockTagSet references.
    pub const fn mines_and_drops(blocks: BlockTagSet, speed: f32) -> Self {
        Self {
            blocks: ToolTagRef::Static(blocks),
            speed: Some(speed),
            correct_for_drops: Some(true),
        }
    }

    /// Creates a tool rule for blocks that deny drops when mined with this tool.
    ///
    /// This is a const function that works with static BlockTagSet references.
    pub const fn denies_drops(blocks: BlockTagSet) -> Self {
        Self {
            blocks: ToolTagRef::Static(blocks),
            speed: None,
            correct_for_drops: Some(false),
        }
    }

    /// Creates a tool rule for blocks that can be mined at the given speed and drop items,
    /// using a dynamic tag identifier for runtime lookup.
    ///
    /// The identifier should be in the format "namespace:path" (e.g., "minecraft:mineable/pickaxe").
    pub const fn mines_and_drops_dynamic(tag_ident: &'static str, speed: f32) -> Self {
        Self {
            blocks: ToolTagRef::DynamicIdent(tag_ident),
            speed: Some(speed),
            correct_for_drops: Some(true),
        }
    }

    /// Creates a tool rule for blocks that deny drops when mined with this tool,
    /// using a dynamic tag identifier for runtime lookup.
    ///
    /// The identifier should be in the format "namespace:path" (e.g., "minecraft:mineable/pickaxe").
    pub const fn denies_drops_dynamic(tag_ident: &'static str) -> Self {
        Self {
            blocks: ToolTagRef::DynamicIdent(tag_ident),
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
