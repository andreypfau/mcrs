use glam::DVec3;
use mcrs_minecraft_physics::Aabb;

/// The box in which a player with the box `player` collects item entities; `vehicle` is the box
/// of the vehicle that it rides, when that vehicle is not removed (IEN-PCK-001). The host tests
/// it against the box of each item entity with a strict intersection.
#[must_use]
pub fn pickup_box(player: Aabb, vehicle: Option<Aabb>) -> Aabb {
    match vehicle {
        Some(vehicle) => Aabb::new(player.min.min(vehicle.min), player.max.max(vehicle.max))
            .inflate(DVec3::new(1.0, 0.0, 1.0)),
        None => player.inflate(DVec3::new(1.0, 0.5, 1.0)),
    }
}

/// True when the collector with the UUID `collector` can pick up an item entity with
/// `pickup_delay` and `owner` (IEN-PCK-002, conditions 1 and 2). The thrower has no effect.
#[must_use]
pub fn can_pick_up(pickup_delay: i16, owner: Option<u128>, collector: u128) -> bool {
    pickup_delay == 0 && owner.is_none_or(|owner| owner == collector)
}
