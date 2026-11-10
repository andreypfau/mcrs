use core::hash::Hash;
use core::hash::Hasher;

use crate::BlockSet;
use crate::float_key;

/// What a tool does to the blocks of one block set.
#[derive(Clone, Debug)]
pub struct ToolRule {
    pub blocks: BlockSet,
    speed: Option<f32>,
    /// `None` when the rule does not decide if the blocks drop their loot.
    pub correct_for_drops: Option<bool>,
}

impl ToolRule {
    /// A rule that sets no speed and does not decide the drops.
    #[must_use]
    pub const fn new(blocks: BlockSet) -> Self {
        Self {
            blocks,
            speed: None,
            correct_for_drops: None,
        }
    }

    /// Gives `None` for a speed that the saved form does not hold: zero, a negative number, an
    /// infinity or a NaN.
    #[must_use]
    pub fn with_speed(mut self, speed: f32) -> Option<Self> {
        if speed > 0.0 && speed.is_finite() {
            self.speed = Some(speed);
            Some(self)
        } else {
            None
        }
    }

    /// The mining speed for the blocks, or `None` when the rule sets no speed.
    #[must_use]
    pub const fn speed(&self) -> Option<f32> {
        self.speed
    }
}

impl PartialEq for ToolRule {
    fn eq(&self, other: &Self) -> bool {
        self.blocks == other.blocks
            && self.speed.map(float_key::of_f32) == other.speed.map(float_key::of_f32)
            && self.correct_for_drops == other.correct_for_drops
    }
}

impl Eq for ToolRule {}

impl Hash for ToolRule {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.blocks.hash(state);
        self.speed.map(float_key::of_f32).hash(state);
        self.correct_for_drops.hash(state);
    }
}
