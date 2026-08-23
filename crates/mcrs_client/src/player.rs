use bevy::core_pipeline::tonemapping::Tonemapping;
use bevy::input::mouse::AccumulatedMouseMotion;
use bevy::prelude::*;
use bevy::render::view::Msaa;
use bevy::window::{CursorGrabMode, CursorOptions, PrimaryWindow};

const EYE_HEIGHT: f32 = 1.62;

const LOOK_SENSITIVITY: f32 = 0.15;

#[derive(Component)]
pub struct Player;

#[derive(Component)]
pub struct PlayerCamera;

/// Minecraft yaw and pitch in degrees. Truth; both `Transform::rotation`s are
/// derived from it.
#[derive(Component, Debug, Clone, Copy)]
pub struct PlayerLook {
    pub yaw: f32,
    pub pitch: f32,
}

/// `mouse_look` is off for a scripted run, where a stray mouse delta would turn
/// the camera away from the view the save asked for.
pub struct PlayerPlugin {
    pub mouse_look: bool,
}

impl Plugin for PlayerPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, (grab_cursor, sync_look_transforms))
            .add_systems(Update, (release_cursor_on_escape, sync_look_transforms).chain());
        if self.mouse_look {
            app.add_systems(Update, apply_mouse_look.before(sync_look_transforms));
        }
    }
}

pub fn spawn_player(world: &mut World, translation: Vec3, yaw: f32, pitch: f32) {
    let player = world
        .spawn((Player, PlayerLook { yaw, pitch }, Transform::from_translation(translation)))
        .id();
    world.spawn((
        PlayerCamera,
        Camera3d::default(),
        // The sky pipelines are built for a single sample; multisampling the
        // view would leave them unable to render into it.
        Msaa::Off,
        // The default `TonyMcMapFace` tonemapper needs the `tonemapping_luts`
        // feature, which nothing else here requires.
        Tonemapping::None,
        Transform::from_xyz(0.0, EYE_HEIGHT, 0.0),
        ChildOf(player),
    ));
}

fn grab_cursor(mut window: Single<&mut CursorOptions, With<PrimaryWindow>>) {
    window.grab_mode = CursorGrabMode::Locked;
    window.visible = false;
}

fn release_cursor_on_escape(
    keys: Res<ButtonInput<KeyCode>>,
    mut window: Single<&mut CursorOptions, With<PrimaryWindow>>,
) {
    if keys.just_pressed(KeyCode::Escape) {
        window.grab_mode = CursorGrabMode::None;
        window.visible = true;
    }
}

fn apply_mouse_look(
    motion: Res<AccumulatedMouseMotion>,
    window: Single<&CursorOptions, With<PrimaryWindow>>,
    mut look: Single<&mut PlayerLook>,
) {
    if window.grab_mode == CursorGrabMode::None || motion.delta == Vec2::ZERO {
        return;
    }
    look.yaw = wrap_yaw(look.yaw + motion.delta.x * LOOK_SENSITIVITY);
    look.pitch = (look.pitch + motion.delta.y * LOOK_SENSITIVITY).clamp(-90.0, 90.0);
}

fn wrap_yaw(yaw: f32) -> f32 {
    (yaw + 180.0).rem_euclid(360.0) - 180.0
}

/// The half-turn is not decoration: Minecraft measures the look direction off
/// +Z while Bevy's camera looks down -Z, and the pitch sign flips with it.
#[allow(clippy::type_complexity)]
fn sync_look_transforms(
    player: Single<(&PlayerLook, &mut Transform), (With<Player>, Without<PlayerCamera>, Changed<PlayerLook>)>,
    mut camera: Single<&mut Transform, (With<PlayerCamera>, Without<Player>)>,
) {
    let (look, mut player_transform) = player.into_inner();
    player_transform.rotation = yaw_rotation(look.yaw);
    camera.rotation = pitch_rotation(look.pitch);
}

fn yaw_rotation(yaw: f32) -> Quat {
    Quat::from_rotation_y(std::f32::consts::PI - yaw.to_radians())
}

fn pitch_rotation(pitch: f32) -> Quat {
    Quat::from_rotation_x(-pitch.to_radians())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn minecraft_look_direction(yaw: f32, pitch: f32) -> Vec3 {
        let (pitch, yaw) = (pitch.to_radians(), -yaw.to_radians());
        Vec3::new(
            yaw.sin() * pitch.cos(),
            -pitch.sin(),
            yaw.cos() * pitch.cos(),
        )
    }

    #[test]
    fn the_camera_faces_where_minecraft_says_it_does() {
        for (yaw, pitch) in [
            (0.0, 0.0),
            (-139.9493, 8.099984),
            (90.0, -30.0),
            (180.0, 45.0),
            (-90.0, 90.0),
            (45.0, -90.0),
        ] {
            let forward = (yaw_rotation(yaw) * pitch_rotation(pitch)) * Vec3::NEG_Z;
            let expected = minecraft_look_direction(yaw, pitch);
            assert!(
                forward.abs_diff_eq(expected, 1e-5),
                "yaw {yaw}, pitch {pitch}: {forward:?} != {expected:?}"
            );
        }
    }
}
