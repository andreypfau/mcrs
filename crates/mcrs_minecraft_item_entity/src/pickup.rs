use glam::DVec3;
use mcrs_minecraft_physics::Aabb;

#[must_use]
pub fn pickup_box(player: Aabb, vehicle: Option<Aabb>) -> Aabb {
    match vehicle {
        Some(vehicle) => Aabb::new(player.min.min(vehicle.min), player.max.max(vehicle.max))
            .inflate(DVec3::new(1.0, 0.0, 1.0)),
        None => player.inflate(DVec3::new(1.0, 0.5, 1.0)),
    }
}

#[must_use]
pub fn can_pick_up(pickup_delay: i16, owner: Option<u128>, collector: u128) -> bool {
    pickup_delay == 0 && owner.is_none_or(|owner| owner == collector)
}
