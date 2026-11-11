use alloc::borrow::Cow;
use core::hash::Hash;
use core::hash::Hasher;

use crate::ToolRule;
use crate::float_key;

/// How the stack mines blocks.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "bevy", derive(bevy_ecs::component::Component))]
pub struct Tool {
    pub rules: Cow<'static, [ToolRule]>,
    pub default_mining_speed: f32,
    pub damage_per_block: i32,
    pub can_destroy_blocks_in_creative: bool,
}

impl Tool {
    /// A tool with the values that the saved form takes for its absent keys: a default mining
    /// speed of 1, a damage of 1 for each block, and blocks that break in creative mode.
    #[must_use]
    pub fn new(rules: impl Into<Cow<'static, [ToolRule]>>) -> Self {
        Self {
            rules: rules.into(),
            default_mining_speed: 1.0,
            damage_per_block: 1,
            can_destroy_blocks_in_creative: true,
        }
    }
}

impl PartialEq for Tool {
    fn eq(&self, other: &Self) -> bool {
        self.rules == other.rules
            && float_key::of_f32(self.default_mining_speed)
                == float_key::of_f32(other.default_mining_speed)
            && self.damage_per_block == other.damage_per_block
            && self.can_destroy_blocks_in_creative == other.can_destroy_blocks_in_creative
    }
}

impl Eq for Tool {}

impl Hash for Tool {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.rules.hash(state);
        float_key::of_f32(self.default_mining_speed).hash(state);
        self.damage_per_block.hash(state);
        self.can_destroy_blocks_in_creative.hash(state);
    }
}
