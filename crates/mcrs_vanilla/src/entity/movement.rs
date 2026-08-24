use bevy_math::{DVec2, DVec3};

/// `LivingEntity.MIN_MOVEMENT_DISTANCE`.
pub const MIN_MOVEMENT_DISTANCE: f64 = 0.003;
/// The air branch of `LivingEntity.travelFlying`.
pub const FLYING_FRICTION: f64 = 0.91;
/// What `Player.travel` decays the saved vertical by instead.
pub const FLYING_VERTICAL_FRICTION: f64 = 0.6;

/// `Entity.getInputVector`. `input` is `(left, forward)` and the rotation is
/// Minecraft's, measured in degrees off `+Z`.
pub fn input_vector(input: DVec2, speed: f64, yaw: f32) -> DVec3 {
    let length_squared = input.length_squared();
    if length_squared < 1e-7 {
        return DVec3::ZERO;
    }
    let scaled = if length_squared > 1.0 {
        input.normalize()
    } else {
        input
    } * speed;
    let (sin, cos) = f64::from(yaw).to_radians().sin_cos();
    DVec3::new(
        scaled.x * cos - scaled.y * sin,
        0.0,
        scaled.y * cos + scaled.x * sin,
    )
}

/// The player branch of the snap in `LivingEntity.tick`: horizontal movement is
/// measured as a pair, so a slow diagonal survives where neither axis would on
/// its own.
pub fn snap_tiny_movement_of_player(velocity: &mut DVec3) {
    if velocity.x * velocity.x + velocity.z * velocity.z
        < MIN_MOVEMENT_DISTANCE * MIN_MOVEMENT_DISTANCE
    {
        velocity.x = 0.0;
        velocity.z = 0.0;
    }
    velocity.y = snap_tiny_axis(velocity.y);
}

/// The branch `LivingEntity.tick` takes for everything that is not a player.
pub fn snap_tiny_movement(velocity: &mut DVec3) {
    velocity.x = snap_tiny_axis(velocity.x);
    velocity.y = snap_tiny_axis(velocity.y);
    velocity.z = snap_tiny_axis(velocity.z);
}

fn snap_tiny_axis(value: f64) -> f64 {
    if value.abs() < MIN_MOVEMENT_DISTANCE {
        0.0
    } else {
        value
    }
}

/// The air branch of `LivingEntity.travelFlying` with `Player.travel`'s
/// override folded in: the vertical that entered the step decays by
/// `FLYING_VERTICAL_FRICTION` rather than by the flying friction. Returns the
/// displacement `Entity.move` is handed.
pub fn travel_flying(velocity: &mut DVec3, input: DVec2, speed: f64, yaw: f32) -> DVec3 {
    let vertical_before = velocity.y;
    *velocity += input_vector(input, speed, yaw);
    let displacement = *velocity;
    velocity.x *= FLYING_FRICTION;
    velocity.z *= FLYING_FRICTION;
    velocity.y = vertical_before * FLYING_VERTICAL_FRICTION;
    displacement
}
