use bevy::input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll};
use bevy::prelude::*;
use bevy::window::{CursorGrabMode, CursorOptions, PrimaryWindow};
use mcrs_minecraft_engine_world::entity::physics::{OldTransform, Transform as PhysicsTransform};

use mcrs_minecraft_client_gui::menu_screens_plugin::MenuScreensPlugin;
use mcrs_minecraft_client_options::ClientOptions;
use mcrs_minecraft_client_world::attack_input::AttackInput;
use mcrs_minecraft_client_world::local_player::{FlyingSpeed, LocalPlayerTick};
use mcrs_minecraft_client_world::movement_input::MovementInput;
use mcrs_minecraft_client_world::player::Player;
use mcrs_minecraft_client_world::screen::Screen;

const FLY_SPEED_STEP: f64 = 0.05;
const FLY_SPEED_MIN: f64 = 0.0;
const FLY_SPEED_MAX: f64 = 2.0;

/// Degrees of turn for each unit of mouse motion at a turn factor of 1.
const DEGREES_PER_MOTION_UNIT: f32 = 0.15;

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
                    (apply_mouse_look, grab_cursor_on_click).chain(),
                    read_fly_speed_wheel,
                ),
            );
        #[cfg(not(target_family = "wasm"))]
        app.add_systems(Update, toggle_fullscreen);
    }

    fn finish(&self, app: &mut App) {
        if !app.is_plugin_added::<MenuScreensPlugin>() {
            app.add_systems(Update, release_cursor_on_escape.before(apply_mouse_look));
        }
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

/// Without the menu screens, Escape frees the pointer; with a screen open, it closes the screen.
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

/// F11 inverts Fullscreen, also while a screen or an edit box has the keys, but not while F3 is
/// down (MENU-DISP-005).
#[cfg(not(target_family = "wasm"))]
fn toggle_fullscreen(keys: Res<ButtonInput<KeyCode>>, mut options: ResMut<ClientOptions>) {
    if keys.just_pressed(KeyCode::F11) && !keys.pressed(KeyCode::F3) {
        options.fullscreen = !options.fullscreen;
    }
}

/// Degrees of turn for each unit of mouse motion at the sensitivity `sensitivity`
/// (CTRL-TURN-002).
fn degrees_per_motion_unit(sensitivity: f32) -> f32 {
    let factor = 0.6 * sensitivity + 0.2;
    DEGREES_PER_MOTION_UNIT * 8.0 * factor.powi(3)
}

pub(crate) fn apply_mouse_look(
    motion: Res<AccumulatedMouseMotion>,
    options: Res<ClientOptions>,
    window: Single<&CursorOptions, With<PrimaryWindow>>,
    player: Single<(&mut PhysicsTransform, &mut OldTransform), With<Player>>,
) {
    if window.grab_mode == CursorGrabMode::None || motion.delta == Vec2::ZERO {
        return;
    }
    let (mut transform, mut previous) = player.into_inner();
    let turn = motion.delta * degrees_per_motion_unit(options.sensitivity);
    transform.rotation = transform.rotation.turn(turn.x, turn.y);
    // The previous look turns with the current one, so the frame shows the turn at once and
    // only a turn that a tick makes is interpolated.
    previous.0.rotation = previous.rotation.turn(turn.x, turn.y);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_sensitivity_sets_the_degrees_of_each_unit_of_motion() {
        for (sensitivity, degrees) in [(0.0, 0.0096), (0.5, 0.15), (1.0, 0.6144)] {
            let turned = degrees_per_motion_unit(sensitivity);
            assert!(
                (turned - degrees).abs() <= degrees * 1e-6,
                "sensitivity {sensitivity}: {turned} degrees"
            );
        }
    }
}
