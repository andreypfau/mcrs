use bevy_app::App;
use mcrs_minecraft_engine_attributes::PlayerAttributesPlugin;
use mcrs_minecraft_engine_world::entity::player::Player;
use mcrs_minecraft_entity_attribute::AttributeInstances;
use mcrs_minecraft_entity_attribute::attribute_keys::Attribute;

#[test]
fn a_spawned_player_has_the_instances_of_the_type_player() {
    let mut app = App::new();
    app.add_plugins(PlayerAttributesPlugin);
    let player = app.world_mut().spawn(Player).id();

    let instances = app
        .world()
        .get::<AttributeInstances>(player)
        .expect("a player has its attributes from its spawn");
    assert_eq!(instances.value(Attribute::MaxHealth), Some(20.0));
    assert_eq!(
        instances.value(Attribute::FollowRange),
        None,
        "a player has no follow range"
    );
    assert_eq!(instances.set().instances().count(), 0);
}
