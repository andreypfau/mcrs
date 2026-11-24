use bevy_app::{App, FixedUpdate, Plugin};
use bevy_ecs::change_detection::DetectChangesMut;
use bevy_ecs::query::Changed;
use bevy_ecs::schedule::IntoScheduleConfigs;
use bevy_ecs::system::Populated;
use mcrs_minecraft_engine::player_sprinting::PlayerSprinting;
use mcrs_minecraft_entity::SPRINT_MODIFIER;
use mcrs_minecraft_entity_attribute::attribute_keys::Attribute;
use mcrs_minecraft_entity_attribute::{
    AttributeInstances, InstanceChange, InstanceEdit, InstanceModifier,
};

use crate::attribute_sources::{AttributeSources, configure_attribute_sources};

#[derive(Clone, Copy, Debug, Default)]
pub struct SprintAttributePlugin;

impl Plugin for SprintAttributePlugin {
    fn build(&self, app: &mut App) {
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
    mut players: Populated<(&PlayerSprinting, &mut AttributeInstances), Changed<PlayerSprinting>>,
) {
    for (sprinting, mut instances) in &mut players {
        let edit = if sprinting.0 {
            InstanceEdit::Put(InstanceModifier::transient(&SPRINT_MODIFIER))
        } else {
            InstanceEdit::Remove(SPRINT_MODIFIER.id.to_buf())
        };
        if instances
            .bypass_change_detection()
            .apply(Attribute::MovementSpeed, &edit)
            == InstanceChange::Changed
        {
            instances.set_changed();
        }
    }
}
