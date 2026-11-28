use bevy_app::{App, Plugin};
use mcrs_minecraft_engine_world::entity::player::Player;
use mcrs_minecraft_entity::entity_type_keys::EntityType;
use mcrs_minecraft_entity_attribute::{AttributeBases, AttributeInstances};

#[derive(Clone, Copy, Debug, Default)]
pub struct PlayerAttributesPlugin;

impl Plugin for PlayerAttributesPlugin {
    fn build(&self, app: &mut App) {
        if let Err(error) = app
            .try_register_required_components_with::<Player, AttributeInstances>(player_instances)
        {
            tracing::error!("a player does not require its attribute instances: {error}");
        }
    }
}

fn player_instances() -> AttributeInstances {
    AttributeInstances::new(PLAYER_BASES)
}

const PLAYER_BASES: AttributeBases =
    match mcrs_minecraft_entity::attribute_bases(EntityType::Player) {
        Some(bases) => bases,
        None => AttributeBases::new(&[], &[]),
    };

const _: () = assert!(
    mcrs_minecraft_entity::attribute_bases(EntityType::Player).is_some(),
    "a player is a living entity and has bases"
);
