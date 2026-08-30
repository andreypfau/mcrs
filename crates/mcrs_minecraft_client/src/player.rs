use bevy::core_pipeline::tonemapping::Tonemapping;
use bevy::input::mouse::AccumulatedMouseMotion;
use bevy::math::DVec3;
use bevy::prelude::*;
use bevy::render::view::Msaa;
use bevy::window::{CursorGrabMode, CursorOptions, PrimaryWindow};

use mcrs_minecraft_world::entity::player::{Flying, FlyingSpeed};
use mcrs_voxel_world::entity::physics::{
    OldTransform, Rotation, Transform as PhysicsTransform, Velocity,
};

use crate::camera::FovFilter;
use crate::local_player::Sprint;
use crate::options::SENSITIVITY;

pub(crate) const EYE_HEIGHT: f32 = 1.62;

const FAR_PLANE: f32 = 4000.0;

/// `MouseHandler.turnPlayer` builds `sens = (sensitivity * 0.6 + 0.2)^3 * 8` and
/// `Entity.turn` then scales by `0.15`; at the default `sensitivity` the chain
/// collapses to `degrees = pixels * 0.15`.
const LOOK_SENSITIVITY: f32 = {
    let sens = SENSITIVITY * 0.6 + 0.2;
    sens * sens * sens * 8.0 * 0.15
};

#[derive(Component)]
pub struct Player;

#[derive(Component)]
pub struct PlayerCamera;

pub struct PlayerPlugin;

impl Plugin for PlayerPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, sync_look_transforms).add_systems(
            Update,
            (
                release_cursor_on_escape,
                apply_mouse_look,
                grab_cursor_on_click,
                sync_look_transforms,
            )
                .chain(),
        );
    }
}

pub fn spawn_player(world: &mut World, position: DVec3, yaw: f32, pitch: f32) {
    let physics =
        PhysicsTransform::from_translation(position).with_rotation(Rotation::new(yaw, pitch));
    let player = world
        .spawn((
            Player,
            physics,
            OldTransform(physics),
            Velocity(DVec3::ZERO),
            Flying,
            FlyingSpeed::default(),
            Sprint::default(),
            Transform::from_translation(position.as_vec3()),
        ))
        .id();
    world.spawn((
        PlayerCamera,
        FovFilter::default(),
        Projection::Perspective(PerspectiveProjection {
            far: FAR_PLANE,
            ..default()
        }),
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

/// Grabbing after the look has been applied drops the motion the pointer made
/// while it was still free, which would otherwise land as a jump on the frame
/// the player clicks.
fn grab_cursor_on_click(
    buttons: Res<ButtonInput<MouseButton>>,
    mut window: Single<&mut CursorOptions, With<PrimaryWindow>>,
) {
    if buttons.just_pressed(MouseButton::Left) {
        window.grab_mode = CursorGrabMode::Locked;
        window.visible = false;
    }
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
    mut transform: Single<&mut PhysicsTransform, With<Player>>,
) {
    if window.grab_mode == CursorGrabMode::None || motion.delta == Vec2::ZERO {
        return;
    }
    transform.rotation = transform.rotation.turn(
        motion.delta.x * LOOK_SENSITIVITY,
        motion.delta.y * LOOK_SENSITIVITY,
    );
}

/// The half-turn is not decoration: Minecraft measures the look direction off
/// +Z while Bevy's camera looks down -Z, and the pitch sign flips with it.
#[allow(clippy::type_complexity)]
fn sync_look_transforms(
    player: Single<
        (&PhysicsTransform, &mut Transform),
        (
            With<Player>,
            Without<PlayerCamera>,
            Changed<PhysicsTransform>,
        ),
    >,
    mut camera: Single<&mut Transform, (With<PlayerCamera>, Without<Player>)>,
) {
    let (physics, mut player_transform) = player.into_inner();
    player_transform.rotation = yaw_rotation(physics.rotation.yaw());
    camera.rotation = pitch_rotation(physics.rotation.pitch());
}

fn yaw_rotation(yaw: f32) -> Quat {
    Quat::from_rotation_y(std::f32::consts::PI - yaw.to_radians())
}

fn pitch_rotation(pitch: f32) -> Quat {
    Quat::from_rotation_x(-pitch.to_radians())
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    pub(crate) fn minecraft_look_direction(yaw: f32, pitch: f32) -> Vec3 {
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
