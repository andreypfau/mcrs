use mcrs_minecraft_engine_attributes::AttributeChanged;
use mcrs_minecraft_engine_attributes::AttributeTarget;
use mcrs_minecraft_engine_attributes::Attributes;
use mcrs_minecraft_engine_attributes::InstanceEdit;
use mcrs_minecraft_engine_attributes::RowAttributes;
use mcrs_minecraft_entity::InstanceModifier;
use mcrs_minecraft_entity::ModifierKind;
use mcrs_minecraft_entity::attribute_keys::Attribute;
use mcrs_minecraft_entity::entity_type_keys::EntityType;
use mcrs_minecraft_entity_attribute::ModifierOperation;
use mcrs_minecraft_entity_key::EntityId;
use mcrs_minecraft_resource::ResourceLocationBuf;

use crate::fixture::app;
use crate::fixture::despawn_row;
use crate::fixture::edit;
use crate::fixture::spawn_pig;
use crate::fixture::spawn_player;
use crate::fixture::tick;

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
fn a_row_has_an_entry_only_from_its_first_change_until_it_leaves_the_world() {
    let mut app = app();
    let pig = EntityId(7);
    spawn_pig(&mut app, pig);
    let speed = |app: &bevy_app::App| {
        app.world().resource::<RowAttributes>().value(
            pig,
            EntityType::Pig,
            Attribute::MovementSpeed,
        )
    };
    tick(&mut app);
    assert!(app.world().resource::<RowAttributes>().get(pig).is_none());
    assert_eq!(speed(&app), Some(0.25), "the base of the type pig");

    let changed = edit(
        &mut app,
        AttributeTarget::Row(pig),
        Attribute::MovementSpeed,
        InstanceEdit::SetBase(0.375),
    );
    assert_eq!(changed.len(), 1);
    assert!(app.world().resource::<RowAttributes>().get(pig).is_some());
    assert_eq!(speed(&app), Some(0.375));

    despawn_row(&mut app, pig);
    tick(&mut app);
    assert!(
        app.world().resource::<RowAttributes>().get(pig).is_none(),
        "the entry leaves with its row"
    );
}

#[test]
fn an_edit_of_an_attribute_outside_the_set_changes_nothing() {
    let mut app = app();
    let player = spawn_player(&mut app);
    let pig = EntityId(7);
    spawn_pig(&mut app, pig);
    let cases = [
        (AttributeTarget::Player(player), Attribute::FollowRange),
        (AttributeTarget::Row(pig), Attribute::AttackDamage),
    ];
    for (target, attribute) in cases {
        assert_eq!(
            edit(&mut app, target, attribute, add_four()),
            [],
            "{target:?} {attribute:?}"
        );
    }
    let attributes = app
        .world()
        .get::<Attributes>(player)
        .expect("a player has its attributes from its spawn");
    assert_eq!(attributes.value(Attribute::FollowRange), None);
    assert_eq!(attributes.set().instances().count(), 0);
    assert!(app.world().resource::<RowAttributes>().get(pig).is_none());
}
