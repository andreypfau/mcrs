use bevy_app::{App, FixedUpdate, Plugin};
use bevy_ecs::entity::Entity;
use bevy_ecs::message::MessageWriter;
use bevy_ecs::query::Changed;
use bevy_ecs::schedule::IntoScheduleConfigs;
use bevy_ecs::system::Query;
use mcrs_minecraft_engine::player_sprinting::PlayerSprinting;
use mcrs_minecraft_engine_attributes::{
    AttributeEdit, AttributeEdits, AttributeTarget, InstanceEdit,
};
use mcrs_minecraft_engine_world::tick::TickSet;
use mcrs_minecraft_entity::attribute_keys::Attribute;
use mcrs_minecraft_entity::{InstanceModifier, ModifierKind};
use mcrs_minecraft_entity_attribute::ModifierOperation;
use mcrs_minecraft_resource::{ResourceLocation, rl};

use crate::attribute_sources::AttributeSources;

const SPRINTING: ResourceLocation<'static> = rl!("minecraft:sprinting");
const SPRINT_AMOUNT: f32 = 0.3;

/// The sprint state of a player gives its `movement_speed` the modifier `minecraft:sprinting`
/// (ATR-SRC-002). Needs the `EntityAttributesPlugin` of the dimension world.
#[derive(Clone, Copy, Debug, Default)]
pub struct SprintAttributePlugin;

impl Plugin for SprintAttributePlugin {
    fn build(&self, app: &mut App) {
        app.configure_sets(
            FixedUpdate,
            AttributeSources
                .in_set(TickSet::Project)
                .before(AttributeEdits),
        );
        app.add_systems(
            FixedUpdate,
            give_sprint_modifier
                .in_set(AttributeSources)
                .ambiguous_with(AttributeSources),
        );
    }
}

fn give_sprint_modifier(
    players: Query<(Entity, &PlayerSprinting), Changed<PlayerSprinting>>,
    mut edits: MessageWriter<AttributeEdit>,
) {
    for (player, sprinting) in &players {
        let edit = if sprinting.0 {
            InstanceEdit::Put(InstanceModifier::new(
                SPRINTING.to_buf(),
                f64::from(SPRINT_AMOUNT),
                ModifierOperation::AddMultipliedTotal,
                ModifierKind::Transient,
            ))
        } else {
            InstanceEdit::Remove(SPRINTING.to_buf())
        };
        edits.write(AttributeEdit {
            target: AttributeTarget::Player(player),
            attribute: Attribute::MovementSpeed,
            edit,
        });
    }
}
