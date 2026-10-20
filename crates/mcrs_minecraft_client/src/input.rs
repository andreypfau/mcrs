use bevy::input::mouse::AccumulatedMouseScroll;
use bevy::math::DVec2;
use bevy::prelude::*;
use bevy::window::{CursorGrabMode, CursorOptions, PrimaryWindow};

use crate::local_player::FlyingSpeed;
use crate::player::Player;

const FLY_SPEED_STEP: f64 = 0.05;
const FLY_SPEED_MIN: f64 = 0.0;
const FLY_SPEED_MAX: f64 = 2.0;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Input {
    pub forward: bool,
    pub backward: bool,
    pub left: bool,
    pub right: bool,
    pub jump: bool,
    pub shift: bool,
    pub sprint: bool,
}

impl Input {
    pub const EMPTY: Self = Self {
        forward: false,
        backward: false,
        left: false,
        right: false,
        jump: false,
        shift: false,
        sprint: false,
    };
}

pub struct ClientInputPlugin;

impl Plugin for ClientInputPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, read_fly_speed_wheel);
    }
}

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

/// The `(left, forward)` direction of the pressed keys, of length one or zero.
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
