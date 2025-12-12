use crate::world::entity::attribute::{Attribute, LivingAttributesBundle, RangedAttribute};
use bevy_ecs::bundle::Bundle;
use bevy_ecs::prelude::Component;

#[derive(Bundle, Default)]
pub struct PlayerAttributesBundle {
    pub living_attributes: LivingAttributesBundle,
    pub block_break_speed: BlockBreakSpeed,
    pub mining_efficiency: MiningEfficiency,
}

#[derive(Debug, Clone, Copy, PartialEq, Component)]
pub struct BlockBreakSpeed {
    base_value: f32,
}

impl Default for BlockBreakSpeed {
    fn default() -> Self {
        Self { base_value: 1.0 }
    }
}

impl Attribute for BlockBreakSpeed {
    fn base_value(&self) -> f32 {
        self.base_value
    }
}

impl RangedAttribute for BlockBreakSpeed {
    fn min_value() -> f32 {
        0.0
    }

    fn max_value() -> f32 {
        1024.0
    }
}

#[derive(Default, Debug, Clone, Copy, PartialEq, Component)]
pub struct MiningEfficiency {
    base_value: f32,
}

impl Attribute for MiningEfficiency {
    fn base_value(&self) -> f32 {
        self.base_value
    }
}

impl RangedAttribute for MiningEfficiency {
    fn min_value() -> f32 {
        0.0
    }
    fn max_value() -> f32 {
        1024.0
    }
}
