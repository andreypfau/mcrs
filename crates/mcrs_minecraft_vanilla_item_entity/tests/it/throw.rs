use glam::DVec3;
use mcrs_minecraft_entity_key::BirthSource;
use mcrs_minecraft_entity_key::EntityKey;
use mcrs_minecraft_item_entity::Cause;
use mcrs_minecraft_item_entity::HandVelocity;
use mcrs_minecraft_item_entity::ItemEntity;
use mcrs_minecraft_item_entity::Start;
use mcrs_minecraft_vanilla_item_entity::Collect;
use mcrs_minecraft_vanilla_item_entity::Form;
use mcrs_minecraft_vanilla_item_entity::Launch;
use mcrs_voxel::BlockPos;

use crate::fixture::FLOOR;
use crate::fixture::PLAYER;
use crate::fixture::STONE;
use crate::fixture::app;
use crate::fixture::collector;
use crate::fixture::items;
use crate::fixture::launch;
use crate::fixture::move_collector;
use crate::fixture::picked_up;
use crate::fixture::run;
use crate::fixture::slot_count;

/// A player at `feet` throws 10 `stone` straight down, onto 10 `stone` that lie there.
#[test]
fn ien_spn_mrg_pck_a_thrown_stack_lands_merges_with_a_near_stack_and_is_picked_up() {
    let mut app = app();
    let world = app.world_mut();
    let feet = DVec3::new(0.5, FLOOR, 0.5);
    let player = collector(world, DVec3::new(10.5, FLOOR, 0.5), Collect::Survival);
    launch(
        world,
        STONE,
        10,
        Launch {
            source: BirthSource::Entity(EntityKey(PLAYER)),
            form: Form::Hand {
                feet,
                eye_height: 1.62,
                velocity: HandVelocity::Look {
                    yaw: 0.0,
                    pitch: 90.0,
                },
            },
            values: ItemEntity::with_pickup_delay(Cause::Throw.pickup_delay()),
        },
    );
    launch(
        world,
        STONE,
        10,
        Launch {
            source: BirthSource::Block(BlockPos::new(0, 0, 0)),
            form: Form::Fixed(Start {
                position: DVec3::new(0.6, FLOOR, 0.5),
                velocity: DVec3::ZERO,
            }),
            values: ItemEntity::default(),
        },
    );

    run(&mut app, 80);

    let merged = items(app.world_mut());
    let [(_, position, count, item)] = merged[..] else {
        panic!("the two stacks merge into one: {merged:?}");
    };
    assert_eq!(count, 20, "the target holds all items (IEN-MRG-005)");
    assert_eq!(position.y, FLOOR, "the thrown stack lands on the floor");
    assert_eq!(item.pickup_delay, 0, "the larger delay counted down");
    assert_eq!(slot_count(app.world(), player, 0), None, "out of reach");

    move_collector(app.world_mut(), player, feet);
    run(&mut app, 1);

    assert!(items(app.world_mut()).is_empty(), "no item entity is left");
    assert_eq!(slot_count(app.world(), player, 0), Some(20), "IEN-PCK-003");
    let picked = picked_up(app.world_mut());
    assert_eq!(
        picked
            .iter()
            .map(|picked| picked.count)
            .collect::<Vec<u8>>(),
        [20],
        "IEN-PCK-004"
    );
}

#[test]
fn ien_spn_002_a_thrown_stack_starts_0_3_below_the_eyes() {
    let mut app = app();
    let world = app.world_mut();
    let feet = DVec3::new(0.5, FLOOR, 0.5);
    launch(
        world,
        STONE,
        1,
        Launch {
            source: BirthSource::Entity(EntityKey(PLAYER)),
            form: Form::Hand {
                feet,
                eye_height: 1.62,
                velocity: HandVelocity::Look {
                    yaw: 0.0,
                    pitch: 0.0,
                },
            },
            values: ItemEntity::with_pickup_delay(40),
        },
    );
    let start = items(world);
    assert_eq!(
        start[0].1.y,
        FLOOR + f64::from(1.62_f32) - f64::from(0.3_f32),
        "the stack is in the world from the launch"
    );
    run(&mut app, 1);
    let after = items(app.world_mut());
    assert!(
        after[0].1.z > start[0].1.z + 0.25,
        "it flies along the look: {after:?}"
    );
    assert_eq!(after[0].3.pickup_delay, 39, "IEN-MOV-002");
}
