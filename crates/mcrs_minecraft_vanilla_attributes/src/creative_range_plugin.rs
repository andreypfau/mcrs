use bevy_app::{App, FixedUpdate, Plugin};
use bevy_ecs::entity::Entity;
use bevy_ecs::message::MessageWriter;
use bevy_ecs::query::Changed;
use bevy_ecs::schedule::IntoScheduleConfigs;
use bevy_ecs::system::Query;
use mcrs_minecraft_engine::ability::PlayerGameMode;
use mcrs_minecraft_engine_attributes::{AttributeEdit, AttributeTarget, InstanceEdit};
use mcrs_minecraft_entity::attribute_keys::Attribute;
use mcrs_minecraft_entity::{InstanceModifier, ModifierKind};
use mcrs_minecraft_entity_attribute::ModifierOperation;
use mcrs_minecraft_game_mode::GameMode;
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

/// Creative mode gives a player the two range modifiers of ATR-SRC-003, from its first tick in
/// creative mode to its first tick in another game mode. Needs the `EntityAttributesPlugin` of
/// the dimension world.
#[derive(Clone, Copy, Debug, Default)]
pub struct CreativeRangePlugin;

impl Plugin for CreativeRangePlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<AttributeEdit>();
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
    players: Query<(Entity, &PlayerGameMode), Changed<PlayerGameMode>>,
    mut edits: MessageWriter<AttributeEdit>,
) {
    for (player, mode) in &players {
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
            edits.write(AttributeEdit {
                target: AttributeTarget::Player(player),
                attribute: range.attribute,
                edit,
            });
        }
    }
}
