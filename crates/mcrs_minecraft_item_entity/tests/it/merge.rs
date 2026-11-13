use glam::DVec3;
use mcrs_minecraft_item_entity::ItemEntity;
use mcrs_minecraft_item_entity::NEVER_AGES;
use mcrs_minecraft_item_entity::NEVER_PICKED_UP;
use mcrs_minecraft_item_entity::Target;
use mcrs_minecraft_item_entity::merge_box;
use mcrs_minecraft_item_entity::merge_due;
use mcrs_minecraft_item_entity::merge_target;
use mcrs_minecraft_physics::Aabb;

fn item_box(at: DVec3) -> Aabb {
    Aabb::of_body(at, 0.25, 0.25)
}

#[test]
fn ien_mrg_001_an_item_looks_every_2_ticks_after_a_block_move_else_every_40() {
    let due = |moved| (1..=80).filter(|&tick| merge_due(tick, moved)).count();
    assert_eq!([due(true), due(false)], [40, 2]);
}

#[test]
fn ien_mrg_002_the_pickup_delay_32767_the_age_minus_32768_and_a_full_stack_do_not_merge() {
    let cases = [
        (ItemEntity::default(), 10, true),
        (ItemEntity::with_pickup_delay(NEVER_PICKED_UP), 10, false),
        (
            ItemEntity {
                age: NEVER_AGES,
                ..ItemEntity::default()
            },
            10,
            false,
        ),
        (ItemEntity::default(), 64, false),
    ];
    for (item, count, merges) in cases {
        assert_eq!(item.can_merge(count, 64), merges, "{item:?} with {count}");
    }
}

#[test]
fn ien_mrg_003_partners_are_within_0_75_on_x_and_0_25_on_y() {
    let origin = DVec3::new(0.5, -60.0, 0.5);
    let cases = [
        (DVec3::new(0.74, 0.0, 0.0), true),
        (DVec3::new(0.76, 0.0, 0.0), false),
        (DVec3::new(0.0, 0.24, 0.0), true),
        (DVec3::new(0.0, 0.26, 0.0), false),
    ];
    for (offset, partners) in cases {
        let found = merge_box(item_box(origin)).intersects(&item_box(origin + offset));
        assert_eq!(found, partners, "offset {offset}");
    }
}

#[test]
fn ien_mrg_005_the_larger_count_takes_the_larger_delay_and_the_smaller_age() {
    assert_eq!(merge_target(10, 20), Target::Second);
    assert_eq!(merge_target(20, 10), Target::First);
    let mut target = ItemEntity {
        age: 7,
        pickup_delay: 5,
        ..ItemEntity::default()
    };
    target.absorb(ItemEntity {
        age: 100,
        pickup_delay: 30,
        ..ItemEntity::default()
    });
    assert_eq!([target.age, target.pickup_delay], [7, 30]);
}
