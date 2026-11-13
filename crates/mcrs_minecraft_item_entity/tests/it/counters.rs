use mcrs_minecraft_item_entity::ItemEntity;
use mcrs_minecraft_item_entity::Lifetime;
use mcrs_minecraft_item_entity::NEVER_AGES;
use mcrs_minecraft_item_entity::NEVER_PICKED_UP;

fn ticks_until_end(mut item: ItemEntity, limit: u32) -> Option<u32> {
    (1..=limit).find(|_| item.grow() == Lifetime::Ends)
}

#[test]
fn ien_age_001_the_age_ends_the_item_entity_at_6000_and_minus_32768_stays() {
    let cases = [(5990, Some(10)), (0, Some(6000)), (NEVER_AGES, None)];
    for (age, ends) in cases {
        let item = ItemEntity {
            age,
            ..ItemEntity::default()
        };
        assert_eq!(ticks_until_end(item, 7000), ends, "age {age}");
    }
}

#[test]
fn ien_mov_002_the_pickup_delay_counts_down_to_0_and_32767_stays() {
    let mut counting = ItemEntity::with_pickup_delay(5);
    let mut fixed = ItemEntity::with_pickup_delay(NEVER_PICKED_UP);
    let delays: Vec<i16> = (0..6)
        .map(|_| {
            counting.count_down();
            fixed.count_down();
            counting.pickup_delay
        })
        .collect();
    assert_eq!(delays, [4, 3, 2, 1, 0, 0]);
    assert_eq!(fixed.pickup_delay, NEVER_PICKED_UP);
}

#[test]
fn ien_dmg_001_002_a_damage_cuts_the_health_toward_zero_and_ends_it_at_0() {
    let cases: [(i16, f32, u32); 5] = [
        (5, 4.0, 2),
        (5, 2.0, 3),
        (5, 3.0, 2),
        (5, 1.0, 5),
        (2, 1.0, 2),
    ];
    for (health, per_tick, removal) in cases {
        let mut item = ItemEntity {
            health,
            ..ItemEntity::default()
        };
        let tick = (1..=10).find(|_| item.damage(per_tick) == Lifetime::Ends);
        assert_eq!(tick, Some(removal), "health {health}, damage {per_tick}");
    }
    let mut item = ItemEntity::default();
    assert_eq!(item.damage(0.5), Lifetime::Lives);
    assert_eq!(item.health, 4, "4.5 becomes 4");
}
