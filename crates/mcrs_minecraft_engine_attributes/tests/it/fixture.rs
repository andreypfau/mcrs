use bevy_app::{App, FixedUpdate};
use bevy_ecs::entity::Entity;
use bevy_ecs::message::Messages;
use mcrs_minecraft_engine_attributes::{
    AttributeChanged, AttributeEdit, AttributeTarget, PlayerAttributesPlugin,
};
use mcrs_minecraft_engine_world::entity::player::Player;
use mcrs_minecraft_entity_attribute::InstanceEdit;
use mcrs_minecraft_entity_attribute::attribute_keys::Attribute;

pub fn app() -> App {
    let mut app = App::new();
    app.add_plugins(PlayerAttributesPlugin);
    app
}

pub fn spawn_player(app: &mut App) -> Entity {
    app.world_mut().spawn(Player).id()
}

/// Writes `edit`, runs one tick and gives the instances that the tick announced as changed.
pub fn edit(
    app: &mut App,
    target: AttributeTarget,
    attribute: Attribute,
    edit: InstanceEdit,
) -> Vec<AttributeChanged> {
    app.world_mut().write_message(AttributeEdit {
        target,
        attribute,
        edit,
    });
    app.world_mut().run_schedule(FixedUpdate);
    app.world_mut()
        .resource_mut::<Messages<AttributeChanged>>()
        .drain()
        .collect()
}
