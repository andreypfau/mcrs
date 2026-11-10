use bevy::input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll};
use bevy::prelude::*;
use bevy::window::{CursorGrabMode, CursorOptions, PrimaryWindow};
use mcrs_minecraft_engine_world::entity::physics::{OldTransform, Transform as PhysicsTransform};

use mcrs_minecraft_client_world::attack_input::AttackInput;
use mcrs_minecraft_client_world::local_player::{FlyingSpeed, LocalPlayerTick};
use mcrs_minecraft_client_world::movement_input::MovementInput;
use mcrs_minecraft_client_world::player::Player;
use mcrs_minecraft_client_world::screen::Screen;

const FLY_SPEED_STEP: f64 = 0.05;
const FLY_SPEED_MIN: f64 = 0.0;
const FLY_SPEED_MAX: f64 = 2.0;

/// Degrees of turn per pixel of mouse motion.
const LOOK_SENSITIVITY: f32 = 0.15;

pub struct ClientInputPlugin;

impl Plugin for ClientInputPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<AttackInput>()
            .add_systems(FixedUpdate, read_movement_keys.before(LocalPlayerTick))
            .add_systems(
                RunFixedMainLoop,
                read_attack_input.in_set(RunFixedMainLoopSystems::BeforeFixedMainLoop),
            )
            .add_systems(
                Update,
                (
                    (
                        release_cursor_on_escape,
                        apply_mouse_look,
                        grab_cursor_on_click,
                    )
                        .chain(),
                    read_fly_speed_wheel,
                ),
            );
    }
}

pub(crate) fn read_movement_keys(
    keys: Res<ButtonInput<KeyCode>>,
    cursor: Single<&CursorOptions, With<PrimaryWindow>>,
    mut input: ResMut<MovementInput>,
) {
    input.set_if_neq(pressed(&keys, &cursor));
}

fn pressed(keys: &ButtonInput<KeyCode>, cursor: &CursorOptions) -> MovementInput {
    if cursor.grab_mode == CursorGrabMode::None {
        return MovementInput::EMPTY;
    }
    MovementInput {
        forward: keys.pressed(KeyCode::KeyW),
        backward: keys.pressed(KeyCode::KeyS),
        left: keys.pressed(KeyCode::KeyA),
        right: keys.pressed(KeyCode::KeyD),
        jump: keys.pressed(KeyCode::Space),
        shift: keys.pressed(KeyCode::ShiftLeft),
        sprint: keys.pressed(KeyCode::ControlLeft),
    }
}

/// Runs before the ticks of the frame and before the click that grabs the cursor, so that
/// click is no press. A frame can run no tick or several, so the presses wait for the next one.
fn read_attack_input(
    buttons: Res<ButtonInput<MouseButton>>,
    screen: Option<Res<Screen>>,
    cursor: Single<&CursorOptions, With<PrimaryWindow>>,
    mut input: ResMut<AttackInput>,
) {
    let screen_open = cursor.grab_mode == CursorGrabMode::None || !screen_is_none(screen);
    input.screen_open = screen_open;
    input.down = !screen_open && buttons.pressed(MouseButton::Left);
    if !screen_open && buttons.just_pressed(MouseButton::Left) {
        input.presses = input.presses.saturating_add(1);
    }
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

/// Grabbing after the look has been applied drops the motion the pointer made
/// while it was still free, which would otherwise land as a jump on the frame
/// the player clicks.
fn grab_cursor_on_click(
    buttons: Res<ButtonInput<MouseButton>>,
    screen: Option<Res<Screen>>,
    mut window: Single<&mut CursorOptions, With<PrimaryWindow>>,
) {
    if buttons.just_pressed(MouseButton::Left) && screen_is_none(screen) {
        window.grab_mode = CursorGrabMode::Locked;
        window.visible = false;
    }
}

/// With a screen open, Escape closes the screen instead.
fn release_cursor_on_escape(
    keys: Res<ButtonInput<KeyCode>>,
    screen: Option<Res<Screen>>,
    mut window: Single<&mut CursorOptions, With<PrimaryWindow>>,
) {
    if keys.just_pressed(KeyCode::Escape) && screen_is_none(screen) {
        window.grab_mode = CursorGrabMode::None;
        window.visible = true;
    }
}

fn screen_is_none(screen: Option<Res<Screen>>) -> bool {
    screen.is_none_or(|screen| *screen == Screen::None)
}

pub(crate) fn apply_mouse_look(
    motion: Res<AccumulatedMouseMotion>,
    window: Single<&CursorOptions, With<PrimaryWindow>>,
    player: Single<(&mut PhysicsTransform, &mut OldTransform), With<Player>>,
) {
    if window.grab_mode == CursorGrabMode::None || motion.delta == Vec2::ZERO {
        return;
    }
    let (mut transform, mut previous) = player.into_inner();
    let before = transform.rotation;
    let yaw = motion.delta.x * LOOK_SENSITIVITY;
    transform.rotation = before.turn(yaw, motion.delta.y * LOOK_SENSITIVITY);
    // The previous look turns with the current one, so the frame shows the turn at once and
    // only a turn that a tick makes is interpolated.
    let pitch = transform.rotation.pitch() - before.pitch();
    previous.0.rotation = previous.rotation.turn(yaw, pitch);
}
