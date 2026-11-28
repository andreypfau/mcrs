use mcrs_minecraft_damage::DamageTags;
use mcrs_minecraft_damage::Defence;
use mcrs_minecraft_engine_physics::TestScene;
use mcrs_minecraft_entity::EntityTypeData;
use mcrs_minecraft_entity_attribute::InstanceEdit;
use mcrs_minecraft_entity_attribute::attribute_keys::Attribute;
use mcrs_minecraft_game_registries::registry_load::test_registries;
use mcrs_minecraft_registries::CurrentRegistries;

use crate::fixture;

struct Case {
    kind: &'static EntityTypeData,
    immune: DamageTags,
}

const CASES: [Case; 3] = [
    Case {
        kind: &EntityTypeData::PIG,
        immune: DamageTags::empty(),
    },
    Case {
        kind: &EntityTypeData::ZOMBIFIED_PIGLIN,
        immune: DamageTags::IS_FIRE,
    },
    Case {
        kind: &EntityTypeData::CHICKEN,
        immune: DamageTags::IS_FALL,
    },
];

#[test]
fn a_mob_spawns_immune_to_fire_by_its_type_and_to_falls_by_the_tag() {
    let mut scene = fixture::scene();
    scene
        .app
        .insert_resource(CurrentRegistries::new(test_registries().clone()));
    for (uuid, case) in (1..).zip(CASES) {
        let mob = fixture::spawn(&mut scene, case.kind, uuid);
        let immune = scene
            .app
            .world()
            .get::<Defence>(mob)
            .map(|defence| defence.immune);
        assert_eq!(
            immune,
            Some(case.immune),
            "{:?} has the immunities of its type",
            case.kind.entity_type
        );
    }
}

#[test]
fn the_defence_of_a_mob_follows_its_armor_attributes() {
    let mut scene = fixture::scene();
    let mob = fixture::spawn(&mut scene, &EntityTypeData::ZOMBIFIED_PIGLIN, 1);
    let defence = |scene: &TestScene| {
        scene
            .app
            .world()
            .get::<Defence>(mob)
            .map(|defence| (defence.armor, defence.toughness, defence.immune))
    };
    assert_eq!(
        defence(&scene),
        Some((2.0, 0.0, DamageTags::IS_FIRE)),
        "the base armor of the type"
    );
    fixture::edit(
        &mut scene,
        mob,
        Attribute::Armor,
        InstanceEdit::SetBase(5.0),
    );
    fixture::edit(
        &mut scene,
        mob,
        Attribute::ArmorToughness,
        InstanceEdit::SetBase(3.0),
    );
    assert_eq!(defence(&scene), Some((5.0, 3.0, DamageTags::IS_FIRE)));
}
