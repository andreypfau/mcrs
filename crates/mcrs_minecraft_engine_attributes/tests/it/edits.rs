use mcrs_minecraft_engine_attributes::{
    AttributeChanged, AttributeTarget, Attributes, InstanceEdit,
};
use mcrs_minecraft_entity::attribute_keys::Attribute;
use mcrs_minecraft_entity::{InstanceModifier, ModifierKind};
use mcrs_minecraft_entity_attribute::ModifierOperation;
use mcrs_minecraft_resource::ResourceLocationBuf;

use crate::fixture::{app, edit, spawn_player};

fn id() -> ResourceLocationBuf {
    "test:a".parse().expect("a valid identifier")
}

fn add_four() -> InstanceEdit {
    InstanceEdit::Put(InstanceModifier::new(
        id(),
        4.0,
        ModifierOperation::AddValue,
        ModifierKind::Transient,
    ))
}

#[test]
fn a_put_and_a_remove_on_a_player_each_announce_one_change_and_an_equal_put_none() {
    let mut app = app();
    let player = spawn_player(&mut app);
    let target = AttributeTarget::Player(player);
    let max_health = |app: &bevy_app::App| {
        app.world()
            .get::<Attributes>(player)
            .expect("a player has its attributes from its spawn")
            .value(Attribute::MaxHealth)
    };
    let changed = vec![AttributeChanged {
        target,
        attribute: Attribute::MaxHealth,
    }];

    assert_eq!(
        edit(&mut app, target, Attribute::MaxHealth, add_four()),
        changed
    );
    assert_eq!(max_health(&app), Some(24.0), "the default 20 and 4");

    assert_eq!(
        edit(&mut app, target, Attribute::MaxHealth, add_four()),
        [],
        "an equal modifier leaves the instance as it is"
    );
    assert_eq!(max_health(&app), Some(24.0));

    assert_eq!(
        edit(
            &mut app,
            target,
            Attribute::MaxHealth,
            InstanceEdit::Remove(id())
        ),
        changed
    );
    assert_eq!(max_health(&app), Some(20.0));
}

#[test]
fn an_edit_of_an_attribute_outside_the_set_changes_nothing() {
    let mut app = app();
    let player = spawn_player(&mut app);
    assert_eq!(
        edit(
            &mut app,
            AttributeTarget::Player(player),
            Attribute::FollowRange,
            add_four()
        ),
        []
    );
    let attributes = app
        .world()
        .get::<Attributes>(player)
        .expect("a player has its attributes from its spawn");
    assert_eq!(attributes.value(Attribute::FollowRange), None);
    assert_eq!(attributes.set().instances().count(), 0);
}
