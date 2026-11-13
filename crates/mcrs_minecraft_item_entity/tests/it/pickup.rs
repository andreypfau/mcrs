use glam::DVec3;
use mcrs_minecraft_item_entity::can_pick_up;
use mcrs_minecraft_item_entity::pickup_box;
use mcrs_minecraft_physics::Aabb;

#[test]
fn ien_pck_001_a_standing_player_reaches_1_40_but_not_1_45_and_2_15_up_but_not_2_30() {
    let feet = DVec3::new(0.5, -60.0, 0.5);
    let reach = pickup_box(Aabb::of_body(feet, 0.6, 1.8), None);
    let cases = [
        (DVec3::new(1.40, 0.0, 0.0), true),
        (DVec3::new(1.45, 0.0, 0.0), false),
        (DVec3::new(1.40, 0.0, 1.40), true),
        (DVec3::new(0.0, 2.15, 0.0), true),
        (DVec3::new(0.0, 2.30, 0.0), false),
    ];
    for (offset, reached) in cases {
        let item = Aabb::of_body(feet + offset, 0.25, 0.25);
        assert_eq!(reach.intersects(&item), reached, "offset {offset}");
    }
}

#[test]
fn ien_pck_001_a_passenger_reaches_around_its_vehicle_with_no_vertical_growth() {
    let player = Aabb::of_body(DVec3::new(0.0, 1.0, 0.0), 0.6, 1.8);
    let vehicle = Aabb::of_body(DVec3::new(3.0, 0.0, 0.0), 1.4, 0.5);
    let reach = pickup_box(player, Some(vehicle));
    assert_eq!(reach.max.x, vehicle.max.x + 1.0);
    assert_eq!([reach.min.y, reach.max.y], [vehicle.min.y, player.max.y]);
}

#[test]
fn ien_pck_002_only_the_owner_picks_up_and_only_after_the_delay() {
    let (player, other) = (1, 2);
    let cases = [
        (0, None, true),
        (0, Some(player), true),
        (0, Some(other), false),
        (1, None, false),
    ];
    for (delay, owner, picked) in cases {
        assert_eq!(
            can_pick_up(delay, owner, player),
            picked,
            "{delay} {owner:?}"
        );
    }
}
