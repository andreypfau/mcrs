use bevy::input::mouse::{AccumulatedMouseMotion, MouseWheel};
use bevy::prelude::*;
use bevy::window::{CursorGrabMode, CursorOptions, PrimaryWindow};
use mcrs_minecraft_engine_world::entity::physics::{OldTransform, Transform as PhysicsTransform};
use mcrs_minecraft_game_mode::GameMode;
use mcrs_minecraft_item::{LAST_HOTBAR_SLOT, SelectedHotbarSlot};

use mcrs_minecraft_client_gui::item_key_codes;
use mcrs_minecraft_client_gui::menu_screens_plugin::MenuScreensPlugin;
use mcrs_minecraft_client_gui::wheel_notches;
use mcrs_minecraft_client_options::ClientOptions;
use mcrs_minecraft_client_world::attack_input::AttackInput;
use mcrs_minecraft_client_world::game_mode::LocalGameMode;
use mcrs_minecraft_client_world::local_player::LocalPlayerTick;
use mcrs_minecraft_client_world::movement_input::MovementInput;
use mcrs_minecraft_client_world::player::Player;
use mcrs_minecraft_client_world::screen::Screen;
use mcrs_minecraft_game_mode::Abilities;

const FLY_SPEED_PER_NOTCH: f32 = 0.005;
const FLY_SPEED_MIN: f32 = 0.0;
const FLY_SPEED_MAX: f32 = 0.2;

const DEGREES_PER_MOTION_UNIT: f32 = 0.15;

const TOOLBAR_ACTIVATORS: [KeyCode; 2] = [KeyCode::KeyC, KeyCode::KeyX];

pub struct ClientInputPlugin;

impl Plugin for ClientInputPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<AttackInput>()
            .add_systems(FixedUpdate, read_movement_keys.before(LocalPlayerTick))
            .add_systems(
                RunFixedMainLoop,
                (
                    read_attack_input,
                    read_hotbar_keys.run_if(cursor_grabbed_without_screen),
                    read_wheel,
                )
                    .in_set(RunFixedMainLoopSystems::BeforeFixedMainLoop),
            )
            .add_systems(Update, (apply_mouse_look, grab_cursor_on_click).chain());
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

#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct WheelSums {
    horizontal: f64,
    vertical: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Notches {
    horizontal: i32,
    vertical: i32,
}

impl WheelSums {
    fn add(&mut self, horizontal: f64, vertical: f64) -> Notches {
        Notches {
            horizontal: take_notches(&mut self.horizontal, horizontal),
            vertical: take_notches(&mut self.vertical, vertical),
        }
    }
}

fn take_notches(sum: &mut f64, value: f64) -> i32 {
    if value.partial_cmp(&0.0) != (*sum).partial_cmp(&0.0) {
        *sum = 0.0;
    }
    *sum += value;
    let notches = sum.trunc();
    *sum -= notches;
    notches as i32
}

fn slot_after(slot: u8, wheel: i32) -> u8 {
    match wheel.signum() {
        1 => slot.checked_sub(1).unwrap_or(LAST_HOTBAR_SLOT),
        -1 if slot < LAST_HOTBAR_SLOT => slot + 1,
        -1 => 0,
        _ => slot,
    }
}

fn fly_speed_after(speed: f32, vertical_notches: i32) -> f32 {
    (speed + vertical_notches as f32 * FLY_SPEED_PER_NOTCH).clamp(FLY_SPEED_MIN, FLY_SPEED_MAX)
}

fn read_wheel(
    mut wheel: MessageReader<MouseWheel>,
    mut sums: Local<WheelSums>,
    screen: Option<Res<Screen>>,
    window: Single<(&CursorOptions, &Window), With<PrimaryWindow>>,
    mut player: Query<
        (
            &mut Abilities,
            &mut SelectedHotbarSlot,
            Option<&LocalGameMode>,
        ),
        With<Player>,
    >,
) {
    let (cursor, window) = *window;
    let screen_open = cursor.grab_mode == CursorGrabMode::None || !screen_is_none(screen);
    let Ok((mut abilities, mut selected, game_mode)) = player.single_mut() else {
        wheel.clear();
        return;
    };
    if screen_open {
        wheel.clear();
        return;
    }
    let spectator = game_mode.is_some_and(|mode| mode.current == GameMode::Spectator);
    let mut speed = abilities.flying_speed;
    let mut slot = selected.0;
    for event in wheel.read() {
        let per_unit = wheel_notches::notches_per_unit(event.unit, window.scale_factor());
        let notches = sums.add(f64::from(event.x) * per_unit, f64::from(event.y) * per_unit);
        if notches.horizontal == 0 && notches.vertical == 0 {
            continue;
        }
        if spectator {
            speed = fly_speed_after(speed, notches.vertical);
        } else if notches.vertical != 0 {
            slot = slot_after(slot, notches.vertical);
        } else {
            slot = slot_after(slot, -notches.horizontal);
        }
    }
    selected.set_if_neq(SelectedHotbarSlot(slot));
    let next = Abilities {
        flying_speed: speed,
        ..*abilities
    };
    abilities.set_if_neq(next);
}

fn read_hotbar_keys(
    keys: Res<ButtonInput<KeyCode>>,
    mut player: Query<
        (
            &mut SelectedHotbarSlot,
            Option<&Abilities>,
            Option<&LocalGameMode>,
        ),
        With<Player>,
    >,
) {
    let Some(slot) = (0..=LAST_HOTBAR_SLOT)
        .zip(item_key_codes::HOTBAR)
        .find_map(|(slot, key)| keys.just_pressed(key).then_some(slot))
    else {
        return;
    };
    let Ok((mut selected, abilities, game_mode)) = player.single_mut() else {
        return;
    };
    if game_mode.is_some_and(|mode| mode.current == GameMode::Spectator) {
        return;
    }
    if abilities.is_some_and(|abilities| abilities.instant_build)
        && keys.any_pressed(TOOLBAR_ACTIVATORS)
    {
        return;
    }
    selected.set_if_neq(SelectedHotbarSlot(slot));
}

fn grab_cursor_on_click(
    buttons: Res<ButtonInput<MouseButton>>,
    screen: Option<Res<Screen>>,
    players: Query<(), With<Player>>,
    mut window: Single<&mut CursorOptions, With<PrimaryWindow>>,
) {
    if buttons.just_pressed(MouseButton::Left) && !players.is_empty() && screen_is_none(screen) {
        window.grab_mode = CursorGrabMode::Locked;
        window.visible = false;
    }
}

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

pub(crate) fn cursor_grabbed_without_screen(
    screen: Option<Res<Screen>>,
    cursor: Single<&CursorOptions, With<PrimaryWindow>>,
) -> bool {
    cursor.grab_mode != CursorGrabMode::None && screen_is_none(screen)
}

#[cfg(not(target_family = "wasm"))]
fn toggle_fullscreen(keys: Res<ButtonInput<KeyCode>>, mut options: ResMut<ClientOptions>) {
    if keys.just_pressed(KeyCode::F11) && !keys.pressed(KeyCode::F3) {
        options.fullscreen = !options.fullscreen;
    }
}

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
    previous.0.rotation = previous.rotation.turn(turn.x, turn.y);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_notch_steps_the_fly_speed_within_its_limits() {
        for (notch, notches, expected) in [
            (1, 30, 0.199_999_96_f32),
            (1, 31, 0.2_f32),
            (-1, 10, 7.450_581e-9_f32),
            (-1, 11, 0.0_f32),
        ] {
            let speed = (0..notches).fold(0.05_f32, |speed, _| fly_speed_after(speed, notch));
            assert_eq!(speed.to_bits(), expected.to_bits(), "{notches} x {notch}");
        }
    }

    #[test]
    fn a_wheel_sum_gives_whole_notches_and_resets_on_a_sign_change() {
        let cases: [(&str, &[f64], i32, f64); 4] = [
            ("two halves give one notch", &[0.5, 0.5], 1, 0.0),
            ("a sign change resets the sum", &[0.7, -0.2], 0, -0.2),
            ("a fraction below one notch", &[0.3], 0, 0.3),
            ("truncation toward zero", &[-1.5], -1, -0.5),
        ];
        for (name, values, notches, rest) in cases {
            let mut sums = WheelSums::default();
            let total: i32 = values
                .iter()
                .map(|&value| sums.add(0.0, value).vertical)
                .sum();
            assert_eq!((total, sums.vertical), (notches, rest), "{name}");
        }
        let mut sums = WheelSums::default();
        let _notches = sums.add(0.4, 0.0);
        let _notches = sums.add(0.0, 0.0);
        assert_eq!(
            sums.horizontal, 0.0,
            "a horizontal value of 0 resets its sum"
        );
    }

    #[test]
    fn each_notch_moves_the_slot_by_one_and_wraps_at_the_ends() {
        for (slot, wheel, after) in [
            (3, 1, 2),
            (3, -1, 4),
            (0, 1, 8),
            (8, -1, 0),
            (5, 3, 4),
            (5, 0, 5),
        ] {
            assert_eq!(slot_after(slot, wheel), after, "slot {slot}, wheel {wheel}");
        }
    }

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
