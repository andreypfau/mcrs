use bevy_app::App;
use bevy_ecs::entity::Entity;
use glam::DVec3;
use mcrs_minecraft_entity_key::BirthSource;
use mcrs_minecraft_item_entity::ItemEntity;
use mcrs_minecraft_item_entity::Start;
use mcrs_minecraft_vanilla_item_entity::Collect;
use mcrs_minecraft_vanilla_item_entity::Form;
use mcrs_minecraft_vanilla_item_entity::ItemCollector;
use mcrs_minecraft_vanilla_item_entity::Launch;
use mcrs_minecraft_vanilla_item_entity::Owner;
use mcrs_voxel::BlockPos;

use crate::fixture::DIRT;
use crate::fixture::FLOOR;
use crate::fixture::STONE;
use crate::fixture::app;
use crate::fixture::collector;
use crate::fixture::fill_slot;
use crate::fixture::items;
use crate::fixture::launch;
use crate::fixture::picked_up;
use crate::fixture::run;
use crate::fixture::slot_count;

const FEET: DVec3 = DVec3::new(0.5, FLOOR, 0.5);

fn at(position: DVec3) -> Launch {
    Launch {
        source: BirthSource::Block(BlockPos::new(0, 0, 0)),
        form: Form::Fixed(Start {
            position,
            velocity: DVec3::ZERO,
        }),
        values: ItemEntity::default(),
    }
}

/// A player at [`FEET`] whose main slots 0 to 34 hold 64 `dirt`.
fn crowded(mode: Collect) -> (App, Entity) {
    let mut app = app();
    let world = app.world_mut();
    let player = collector(world, FEET, mode);
    for index in 0..35 {
        fill_slot(world, player, index, DIRT, 64);
    }
    (app, player)
}

#[test]
fn ien_pck_005_the_items_without_room_stay_in_the_item_entity() {
    let (mut app, player) = crowded(Collect::Survival);
    fill_slot(app.world_mut(), player, 35, STONE, 60);
    launch(app.world_mut(), STONE, 10, at(FEET));

    run(&mut app, 1);

    assert_eq!(slot_count(app.world(), player, 35), Some(64));
    let left: Vec<u8> = items(app.world_mut()).iter().map(|item| item.2).collect();
    assert_eq!(left, [6], "R17");
    let counts: Vec<u8> = picked_up(app.world_mut())
        .iter()
        .map(|picked| picked.count)
        .collect();
    assert_eq!(counts, [4], "the take has the count of the part that moved");
}

#[test]
fn ien_pck_006_a_creative_collector_destroys_what_finds_no_room() {
    let (mut app, player) = crowded(Collect::Creative);
    fill_slot(app.world_mut(), player, 35, DIRT, 64);
    launch(app.world_mut(), STONE, 10, at(FEET));

    run(&mut app, 1);

    assert!(
        items(app.world_mut()).is_empty(),
        "R18: no item entity after"
    );
    let counts: Vec<u8> = picked_up(app.world_mut())
        .iter()
        .map(|picked| picked.count)
        .collect();
    assert_eq!(counts, [10], "R18: the take has the count 10");
    assert_eq!(
        slot_count(app.world(), player, 35),
        Some(64),
        "no slot change"
    );
}

#[test]
fn ien_mrg_004_item_entities_of_different_owners_do_not_merge() {
    let mut app = app();
    let world = app.world_mut();
    let owned = launch(world, STONE, 10, at(FEET));
    world.entity_mut(owned).insert(Owner(9));
    launch(world, STONE, 20, at(FEET + DVec3::new(0.5, 0.0, 0.0)));

    run(&mut app, 45);

    let counts: Vec<u8> = items(app.world_mut()).iter().map(|item| item.2).collect();
    assert_eq!(counts, [10, 20], "R19");
}

#[test]
fn ien_mrg_006_an_item_entity_takes_part_in_one_merge_in_a_tick() {
    let mut app = app();
    let world = app.world_mut();
    for _ in 0..3 {
        launch(world, STONE, 10, at(FEET));
    }

    let mut piles = vec![3];
    for _ in 0..200 {
        run(&mut app, 1);
        let left = items(app.world_mut()).len();
        if piles.last() != Some(&left) {
            piles.push(left);
        }
    }

    assert_eq!(
        piles,
        [3, 2, 1],
        "one merge in a tick, the next at a later search"
    );
    let counts: Vec<u8> = items(app.world_mut()).iter().map(|item| item.2).collect();
    assert_eq!(counts, [30]);
}

#[test]
fn ien_pck_007_of_two_collectors_in_reach_the_lower_uuid_takes_the_item() {
    let mut app = app();
    let world = app.world_mut();
    let higher = collector(world, FEET, Collect::Survival);
    let lower = collector(world, FEET + DVec3::new(0.5, 0.0, 0.0), Collect::Survival);
    world
        .get_mut::<ItemCollector>(lower)
        .expect("a collector")
        .uuid = 3;
    launch(world, STONE, 10, at(FEET));

    run(&mut app, 1);

    assert_eq!(
        [
            slot_count(app.world(), lower, 0),
            slot_count(app.world(), higher, 0)
        ],
        [Some(10), None]
    );
}
