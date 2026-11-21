use bevy_app::{App, Plugin};
use mcrs_minecraft_engine_world::world::sub_app::AddDimensionPlugins;
use mcrs_minecraft_redstone::{RedstoneRules, WireEvaluator};
use mcrs_minecraft_server_settings::{RedstonePreset, RedstoneSettings};
use mcrs_minecraft_vanilla_redstone::RedstonePlugin;

use crate::VanillaBlocks;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct VanillaRedstonePlugin {
    pub rules: RedstoneRules,
}

impl VanillaRedstonePlugin {
    #[must_use]
    pub fn from_settings(settings: &RedstoneSettings) -> Self {
        let mut rules = match settings.preset {
            RedstonePreset::Java => RedstoneRules::JAVA,
            RedstonePreset::Bedrock => RedstoneRules::BEDROCK,
        };
        if let Some(on) = settings.quasi_connectivity {
            rules.quasi_connectivity = on;
        }
        if let Some(on) = settings.sticky_piston_short_pulse {
            rules.sticky_piston_short_pulse = on;
        }
        if let Some(on) = settings.pistons_move_block_entities {
            rules.pistons_move_block_entities = on;
        }
        if let Some(on) = settings.dust_connects_to_pistons {
            rules.dust_connects_to_pistons = on;
        }
        if let Some(on) = settings.experimental_wire {
            rules.wire_evaluator = if on {
                WireEvaluator::Experimental
            } else {
                WireEvaluator::Default
            };
        }
        Self { rules }
    }
}

impl Plugin for VanillaRedstonePlugin {
    fn build(&self, app: &mut App) {
        app.add_dimension_plugin(RedstonePlugin::<VanillaBlocks>::with_rules(self.rules));
    }
}
