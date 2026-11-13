use glam::DVec3;
use mcrs_minecraft_entity_key::BirthSource;
use mcrs_minecraft_item_entity::ItemEntity;
use mcrs_minecraft_item_entity::NEVER_AGES;
use mcrs_minecraft_item_entity::Start;
use mcrs_minecraft_vanilla_item_entity::Form;
use mcrs_minecraft_vanilla_item_entity::Launch;
use mcrs_voxel::BlockPos;

use crate::fixture::FLOOR;
use crate::fixture::STONE;
use crate::fixture::app;
use crate::fixture::items;
use crate::fixture::launch;
use crate::fixture::run;

fn at_rest(position: DVec3, values: ItemEntity) -> Launch {
    Launch {
        source: BirthSource::Block(BlockPos::new(0, 0, 0)),
        form: Form::Fixed(Start {
            position,
            velocity: DVec3::ZERO,
        }),
        values,
    }
}

#[test]
fn ien_age_001_the_age_5990_ends_in_the_tenth_tick_and_minus_32768_stays() {
    let mut app = app();
    let world = app.world_mut();
    let ending = ItemEntity {
        age: 5990,
        ..ItemEntity::default()
    };
    let staying = ItemEntity {
        age: NEVER_AGES,
        ..ItemEntity::default()
    };
    let ends = launch(
        world,
        STONE,
        1,
        at_rest(DVec3::new(0.5, FLOOR, 0.5), ending),
    );
    let stays = launch(
        world,
        STONE,
        1,
        at_rest(DVec3::new(5.5, FLOOR, 0.5), staying),
    );

    run(&mut app, 9);
    let alive: Vec<_> = items(app.world_mut()).iter().map(|item| item.0).collect();
    assert_eq!(alive, [ends, stays], "both exist after 9 ticks");
    run(&mut app, 1);
    let alive: Vec<_> = items(app.world_mut()).iter().map(|item| item.0).collect();
    assert_eq!(alive, [stays], "R13");
    assert!(
        app.world().get_entity(ends).is_err(),
        "the stack ends with it"
    );
}

#[test]
fn ien_mov_004_an_item_entity_below_the_world_minus_64_is_removed() {
    let mut app = app();
    let world = app.world_mut();
    let kept = launch(
        world,
        STONE,
        1,
        at_rest(DVec3::new(0.5, -127.9, 0.5), ItemEntity::default()),
    );
    launch(
        world,
        STONE,
        1,
        at_rest(DVec3::new(5.5, -128.1, 0.5), ItemEntity::default()),
    );
    run(&mut app, 1);
    let alive: Vec<_> = items(app.world_mut()).iter().map(|item| item.0).collect();
    assert_eq!(alive, [kept], "R12");
}
