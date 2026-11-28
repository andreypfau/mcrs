use bevy_app::{App, FixedUpdate, Plugin};
use bevy_ecs::change_detection::DetectChangesMut;
use bevy_ecs::query::Changed;
use bevy_ecs::schedule::IntoScheduleConfigs;
use bevy_ecs::system::Populated;
use mcrs_minecraft_entity_attribute::attribute_keys::Attribute;
use mcrs_minecraft_entity_attribute::{
    AttributeInstances, InstanceChange, InstanceEdit, InstanceModifier, ModifierKind,
    ModifierOperation,
};
use mcrs_minecraft_game_mode::{GameMode, PlayerGameMode};
use mcrs_minecraft_resource::{ResourceLocation, rl};

use crate::attribute_sources::{AttributeSources, configure_attribute_sources};

struct RangeModifier {
    attribute: Attribute,
    id: ResourceLocation<'static>,
    amount: f64,
}

const CREATIVE_RANGES: [RangeModifier; 2] = [
    RangeModifier {
        attribute: Attribute::BlockInteractionRange,
        id: rl!("minecraft:creative_mode_block_range"),
        amount: 0.5,
    },
    RangeModifier {
        attribute: Attribute::EntityInteractionRange,
        id: rl!("minecraft:creative_mode_entity_range"),
        amount: 2.0,
    },
];

#[derive(Clone, Copy, Debug, Default)]
pub struct CreativeRangePlugin;

impl Plugin for CreativeRangePlugin {
    fn build(&self, app: &mut App) {
        configure_attribute_sources(app);
        app.add_systems(
            FixedUpdate,
            give_creative_ranges
                .in_set(AttributeSources)
                .ambiguous_with(AttributeSources),
        );
    }
}

fn give_creative_ranges(
    mut players: Populated<(&PlayerGameMode, &mut AttributeInstances), Changed<PlayerGameMode>>,
) {
    for (mode, mut instances) in &mut players {
        for range in &CREATIVE_RANGES {
            let edit = if mode.0 == GameMode::Creative {
                InstanceEdit::Put(InstanceModifier::new(
                    range.id.to_buf(),
                    range.amount,
                    ModifierOperation::AddValue,
                    ModifierKind::Transient,
                ))
            } else {
                InstanceEdit::Remove(range.id.to_buf())
            };
            if instances
                .bypass_change_detection()
                .apply(range.attribute, &edit)
                == InstanceChange::Changed
            {
                instances.set_changed();
            }
        }
    }
}
