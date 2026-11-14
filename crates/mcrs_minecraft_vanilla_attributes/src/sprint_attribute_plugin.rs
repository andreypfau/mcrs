use bevy_app::{App, FixedUpdate, Plugin};
use bevy_ecs::entity::Entity;
use bevy_ecs::message::MessageWriter;
use bevy_ecs::query::Changed;
use bevy_ecs::schedule::IntoScheduleConfigs;
use bevy_ecs::system::Query;
use mcrs_minecraft_engine::player_sprinting::PlayerSprinting;
use mcrs_minecraft_engine_attributes::{AttributeEdit, AttributeTarget, InstanceEdit};
use mcrs_minecraft_entity::attribute_keys::Attribute;
use mcrs_minecraft_entity::{InstanceModifier, ModifierKind, SPRINT_MODIFIER};

use crate::attribute_sources::{AttributeSources, configure_attribute_sources};

/// The sprint state of a player gives its `movement_speed` the modifier `minecraft:sprinting`
/// (ATR-SRC-002). Needs the `EntityAttributesPlugin` of the dimension world.
#[derive(Clone, Copy, Debug, Default)]
pub struct SprintAttributePlugin;

impl Plugin for SprintAttributePlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<AttributeEdit>();
        configure_attribute_sources(app);
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
                SPRINT_MODIFIER.id.to_buf(),
                SPRINT_MODIFIER.amount,
                SPRINT_MODIFIER.operation,
                ModifierKind::Transient,
            ))
        } else {
            InstanceEdit::Remove(SPRINT_MODIFIER.id.to_buf())
        };
        edits.write(AttributeEdit {
            target: AttributeTarget::Player(player),
            attribute: Attribute::MovementSpeed,
            edit,
        });
    }
}
