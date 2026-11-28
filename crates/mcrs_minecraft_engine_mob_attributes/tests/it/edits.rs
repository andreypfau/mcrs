use bevy_ecs::change_detection::DetectChangesMut;
use bevy_ecs::entity::Entity;
use mcrs_minecraft_engine_entity_motion::MotionState;
use mcrs_minecraft_engine_mob_attributes::MobAttributes;
use mcrs_minecraft_engine_physics::TestScene;
use mcrs_minecraft_entity::entity_type_keys::EntityType;
use mcrs_minecraft_entity_attribute::attribute_keys::Attribute;
use mcrs_minecraft_entity_attribute::{
    InstanceEdit, InstanceModifier, ModifierKind, ModifierOperation,
};
use mcrs_minecraft_physics::Rest;
use mcrs_minecraft_resource::ResourceLocationBuf;

use crate::fixture::{edit, scene, spawn_pig, tick};

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

fn attributes(scene: &TestScene, pig: Entity) -> Option<&MobAttributes> {
    scene.app.world().get::<MobAttributes>(pig)
}

fn edited(scene: &TestScene, pig: Entity) -> bool {
    attributes(scene, pig)
        .expect("a mob has attributes")
        .set()
        .instances()
        .next()
        .is_some()
}

#[test]
fn a_mob_holds_its_attributes_from_spawn_until_it_leaves_the_world() {
    let mut scene = scene();
    let pig = spawn_pig(&mut scene, 7);
    let speed = |scene: &TestScene| {
        attributes(scene, pig)
            .and_then(|attributes| attributes.value(EntityType::Pig, Attribute::MovementSpeed))
    };
    tick(&mut scene);
    assert!(!edited(&scene, pig));
    assert_eq!(speed(&scene), Some(0.25), "the base of the type pig");

    let changed = edit(
        &mut scene,
        pig,
        Attribute::MovementSpeed,
        InstanceEdit::SetBase(0.375),
    );
    assert_eq!(changed.len(), 1);
    assert!(edited(&scene, pig));
    assert_eq!(speed(&scene), Some(0.375));

    scene.app.world_mut().despawn(pig);
    tick(&mut scene);
    assert!(
        attributes(&scene, pig).is_none(),
        "the attributes leave with their mob"
    );
}

#[test]
fn an_edit_of_an_attribute_outside_the_set_changes_nothing() {
    let mut scene = scene();
    let pig = spawn_pig(&mut scene, 7);
    assert_eq!(
        edit(&mut scene, pig, Attribute::AttackDamage, add_four()),
        []
    );
    assert!(!edited(&scene, pig));
}

#[test]
fn speed_modifier_reaches_motion_and_removal_restores_base() {
    let mut scene = scene();
    let pig = spawn_pig(&mut scene, 7);
    let motion_speed = |scene: &TestScene| {
        scene
            .state(pig)
            .and_then(|state| state.living)
            .expect("a pig is living")
            .attributes
            .movement_speed
    };
    let half_more = InstanceEdit::Put(InstanceModifier::new(
        id(),
        0.5,
        ModifierOperation::AddMultipliedTotal,
        ModifierKind::Transient,
    ));

    edit(&mut scene, pig, Attribute::MovementSpeed, half_more);
    assert_eq!(
        motion_speed(&scene),
        0.375,
        "the base 0.25 of a pig times 1.5"
    );

    edit(
        &mut scene,
        pig,
        Attribute::MovementSpeed,
        InstanceEdit::Remove(id()),
    );
    assert_eq!(motion_speed(&scene), 0.25, "the base of the type pig");
}

#[test]
fn a_new_gravity_wakes_a_resting_mob() {
    let mut scene = scene();
    let pig = spawn_pig(&mut scene, 7);
    let mut state = scene
        .app
        .world_mut()
        .get_mut::<MotionState>(pig)
        .expect("a mob moves");
    let motion = &mut state.bypass_change_detection().0;
    motion.rest = Rest::Resting(motion.body);

    edit(
        &mut scene,
        pig,
        Attribute::Gravity,
        InstanceEdit::SetBase(0.01),
    );

    let rest = scene.state(pig).map(|state| state.rest);
    assert_eq!(rest, Some(Rest::Awake));
}
