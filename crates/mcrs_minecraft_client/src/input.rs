use bevy::input::mouse::AccumulatedMouseScroll;
use bevy::math::DVec2;
use bevy::prelude::*;
use bevy::window::{CursorGrabMode, CursorOptions, PrimaryWindow};
use mcrs_minecraft_world::entity::player::{FlyingSpeed, Input};

use crate::player::Player;

const INPUT_SCALE: f64 = 0.98;
const FLY_SPEED_STEP: f64 = 0.005;
const FLY_SPEED_MIN: f64 = 0.0;
const FLY_SPEED_MAX: f64 = 0.2;

pub struct ClientInputPlugin;

impl Plugin for ClientInputPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, read_fly_speed_wheel);
    }
}

/// `KeyboardInput.tick`'s key bindings.
pub fn pressed(keys: &ButtonInput<KeyCode>, cursor: &CursorOptions) -> Input {
    if cursor.grab_mode == CursorGrabMode::None {
        return Input::EMPTY;
    }
    Input {
        forward: keys.pressed(KeyCode::KeyW),
        backward: keys.pressed(KeyCode::KeyS),
        left: keys.pressed(KeyCode::KeyA),
        right: keys.pressed(KeyCode::KeyD),
        jump: keys.pressed(KeyCode::Space),
        shift: keys.pressed(KeyCode::ShiftLeft),
        sprint: keys.pressed(KeyCode::ControlLeft),
    }
}

/// The `(left, forward)` impulse pair `KeyboardInput.tick` normalizes.
pub fn move_vector(input: Input) -> DVec2 {
    DVec2::new(
        impulse(input.left, input.right),
        impulse(input.forward, input.backward),
    )
    .normalize_or_zero()
}

fn impulse(positive: bool, negative: bool) -> f64 {
    f64::from(i8::from(positive) - i8::from(negative))
}

/// `ClientInput.modifyInput`, through `modifyInputSpeedForSquareMovement`.
pub fn modify_input(move_vector: DVec2) -> DVec2 {
    if move_vector.length_squared() == 0.0 {
        return move_vector;
    }
    let scaled = move_vector * INPUT_SCALE;
    let length = scaled.length();
    let direction = scaled / length;
    direction * (length * distance_to_unit_square(direction)).min(1.0)
}

fn distance_to_unit_square(direction: DVec2) -> f64 {
    let (x, y) = (direction.x.abs(), direction.y.abs());
    let tangent = if y > x { x / y } else { y / x };
    (1.0 + tangent * tangent).sqrt()
}

fn adjust_fly_speed(speed: f64, wheel: f64) -> f64 {
    (speed + wheel * FLY_SPEED_STEP).clamp(FLY_SPEED_MIN, FLY_SPEED_MAX)
}

fn read_fly_speed_wheel(
    scroll: Res<AccumulatedMouseScroll>,
    cursor: Single<&CursorOptions, With<PrimaryWindow>>,
    mut speed: Single<&mut FlyingSpeed, With<Player>>,
) {
    if cursor.grab_mode == CursorGrabMode::None || scroll.delta.y == 0.0 {
        return;
    }
    speed.0 = adjust_fly_speed(speed.0, f64::from(scroll.delta.y));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_wheel_cannot_push_fly_speed_out_of_range() {
        let mut speed = FlyingSpeed::default().0;
        for wheel in [1.0, 1.0, -1.0, 5.0, -100.0, 100.0, -3.0] {
            speed = adjust_fly_speed(speed, wheel);
            assert!((FLY_SPEED_MIN..=FLY_SPEED_MAX).contains(&speed), "{speed}");
        }
    }
}
