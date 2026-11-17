use mcrs_minecraft_engine_mob_attributes::{AttributeTarget, RowAttributes};
use mcrs_minecraft_entity::entity_type_keys::EntityType;
use mcrs_minecraft_entity_attribute::attribute_keys::Attribute;
use mcrs_minecraft_entity_attribute::{
    InstanceEdit, InstanceModifier, ModifierKind, ModifierOperation,
};
use mcrs_minecraft_entity_key::EntityId;
use mcrs_minecraft_entity_rows::{Placed, RowStore};
use mcrs_minecraft_physics::Rest;
use mcrs_minecraft_resource::ResourceLocationBuf;

use crate::fixture::{app, despawn_row, edit, spawn_pig, tick};

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
    let pig = EntityId(7);
    spawn_pig(&mut app, pig);
    assert_eq!(
        edit(
            &mut app,
            AttributeTarget::Row(pig),
            Attribute::AttackDamage,
            add_four()
        ),
        []
    );
    assert!(app.world().resource::<RowAttributes>().get(pig).is_none());
}

#[test]
fn speed_modifier_reaches_motion_and_removal_restores_base() {
    let mut app = app();
    let pig = EntityId(7);
    spawn_pig(&mut app, pig);
    let target = AttributeTarget::Row(pig);
    let motion_speed = |app: &bevy_app::App| {
        let rows = app.world().resource::<RowStore<()>>();
        let tile = rows.tile_of(pig).expect("the row is living");
        rows.state_of(Placed { tile, id: pig })
            .and_then(|state| state.living)
            .expect("a pig row is living")
            .attributes
            .movement_speed
    };
    let half_more = InstanceEdit::Put(InstanceModifier::new(
        id(),
        0.5,
        ModifierOperation::AddMultipliedTotal,
        ModifierKind::Transient,
    ));

    edit(&mut app, target, Attribute::MovementSpeed, half_more);
    assert_eq!(
        motion_speed(&app),
        0.375,
        "the base 0.25 of a pig times 1.5"
    );

    edit(
        &mut app,
        target,
        Attribute::MovementSpeed,
        InstanceEdit::Remove(id()),
    );
    assert_eq!(motion_speed(&app), 0.25, "the base of the type pig");
}

#[test]
fn a_new_gravity_wakes_a_resting_row() {
    let mut app = app();
    let pig = EntityId(7);
    spawn_pig(&mut app, pig);
    let mut rows = app.world_mut().resource_mut::<RowStore<()>>();
    let tile = rows.tile_of(pig).expect("the row is living");
    let state = rows
        .state_mut(Placed { tile, id: pig })
        .expect("the tile holds the row");
    state.rest = Rest::Resting(state.body);

    edit(
        &mut app,
        AttributeTarget::Row(pig),
        Attribute::Gravity,
        InstanceEdit::SetBase(0.01),
    );

    let rows = app.world().resource::<RowStore<()>>();
    let rest = rows
        .state_of(Placed { tile, id: pig })
        .map(|state| state.rest);
    assert_eq!(rest, Some(Rest::Awake));
}
