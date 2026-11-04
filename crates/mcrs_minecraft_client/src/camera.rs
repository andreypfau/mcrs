use bevy::core_pipeline::tonemapping::Tonemapping;
use bevy::math::DVec3;
use bevy::prelude::*;
use bevy::render::render_resource::TextureUsages;
use bevy::render::view::Msaa;

use mcrs_minecraft_engine_world::entity::physics::{OldTransform, Transform as PhysicsTransform};

use mcrs_minecraft_client_world::local_player::{LocalPlayerTick, Sprint};
use mcrs_minecraft_client_world::player::Player;
use mcrs_minecraft_entity::PLAYER_EYE_HEIGHT;
use mcrs_minecraft_render::CameraOrigin;

const FAR_PLANE: f32 = 4000.0;

#[derive(Component)]
pub struct PlayerCamera;

/// The default field of view. The FOV effects scale stays at its default 1.0, so the
/// modifier applies in full.
const FOV: f32 = 70.0;
const FLYING_FOV_MODIFIER: f32 = 1.1;
/// `1.1 * (1.3 + 1) / 2`, where `1.3` is `MOVEMENT_SPEED` scaled by the `+0.3`
/// `ADD_MULTIPLIED_TOTAL` sprint modifier. Hard-coded because this player has no
/// attribute system to look it up in.
const SPRINTING_FOV_MODIFIER: f32 = 1.265;
const FOV_FILTER_RATE: f32 = 0.5;
const FOV_MODIFIER_MIN: f32 = 0.1;
const FOV_MODIFIER_MAX: f32 = 1.5;

#[derive(Component, Debug, Clone, Copy)]
pub struct FovFilter {
    current: f32,
    previous: f32,
}

/// Vanilla's field starts at the Java default of zero, so joining a world zooms
/// the view open over the first few ticks. Starting settled skips that.
impl Default for FovFilter {
    fn default() -> Self {
        Self {
            current: FLYING_FOV_MODIFIER,
            previous: FLYING_FOV_MODIFIER,
        }
    }
}

pub struct CameraPlugin;

impl Plugin for CameraPlugin {
    fn build(&self, app: &mut App) {
        app.add_observer(attach_camera)
            .add_systems(FixedUpdate, tick_fov.after(LocalPlayerTick))
            .add_systems(
                Update,
                (
                    align_render_transforms.after(crate::input::apply_mouse_look),
                    apply_fov,
                ),
            );
    }
}

fn attach_camera(add: On<Add, Player>, physics: Query<&PhysicsTransform>, mut commands: Commands) {
    let player = add.entity;
    let translation = physics
        .get(player)
        .map_or(Vec3::ZERO, |physics| physics.translation.as_vec3());
    commands.entity(player).insert((
        Transform::from_translation(translation),
        // The camera hangs off the player, and visibility only reaches a child through a
        // parent that takes part in it.
        Visibility::default(),
    ));
    commands.spawn((
        PlayerCamera,
        FovFilter::default(),
        Projection::Perspective(PerspectiveProjection {
            far: FAR_PLANE,
            ..default()
        }),
        Camera3d {
            // The cull reads the last frame's depth back as a pyramid.
            depth_texture_usages: (TextureUsages::RENDER_ATTACHMENT
                | TextureUsages::TEXTURE_BINDING)
                .into(),
            ..default()
        },
        // The sky pipelines are built for a single sample; multisampling the
        // view would leave them unable to render into it.
        Msaa::Off,
        // The default `TonyMcMapFace` tonemapper needs the `tonemapping_luts`
        // feature, which nothing else here requires.
        Tonemapping::None,
        Transform::from_xyz(0.0, PLAYER_EYE_HEIGHT as f32, 0.0),
        ChildOf(player),
    ));
}

/// The half-turn is not decoration: Minecraft measures the look direction off
/// +Z while Bevy's camera looks down -Z, and the pitch sign flips with it.
fn yaw_rotation(yaw: f32) -> Quat {
    Quat::from_rotation_y(std::f32::consts::PI - yaw.to_radians())
}

fn pitch_rotation(pitch: f32) -> Quat {
    Quat::from_rotation_x(-pitch.to_radians())
}

fn next_fov_modifier(current: f32, target: f32) -> f32 {
    (current + (target - current) * FOV_FILTER_RATE).clamp(FOV_MODIFIER_MIN, FOV_MODIFIER_MAX)
}

/// Eases the field of view towards the sprint or flight modifier every tick.
fn tick_fov(sprint: Single<&Sprint, With<Player>>, mut fov: Single<&mut FovFilter>) {
    let target = if sprint.active {
        SPRINTING_FOV_MODIFIER
    } else {
        FLYING_FOV_MODIFIER
    };
    fov.previous = fov.current;
    fov.current = next_fov_modifier(fov.current, target);
}

/// Interpolates the player between ticks and turns the camera to its pitch; the only writer of the
/// player's and the camera's render transforms.
fn align_render_transforms(
    time: Res<Time<Fixed>>,
    player: Single<
        (&PhysicsTransform, &OldTransform, &mut Transform),
        (With<Player>, Without<PlayerCamera>),
    >,
    camera: Single<&mut Transform, (With<PlayerCamera>, Without<Player>)>,
    mut origin: ResMut<CameraOrigin>,
) {
    let (physics, old_physics, mut transform) = player.into_inner();
    let translation = old_physics
        .translation
        .lerp(physics.translation, time.overstep_fraction_f64());
    transform.translation = translation.as_vec3();
    transform.rotation = yaw_rotation(physics.rotation.yaw());
    let pitch = pitch_rotation(physics.rotation.pitch());
    let mut camera = camera.into_inner();
    if camera.rotation != pitch {
        camera.rotation = pitch;
    }
    *origin = CameraOrigin::of(translation + DVec3::Y * PLAYER_EYE_HEIGHT);
}

fn apply_fov(
    time: Res<Time<Fixed>>,
    camera: Single<(&FovFilter, &mut Projection), With<PlayerCamera>>,
) {
    let (fov, projection) = camera.into_inner();
    if let Projection::Perspective(perspective) = projection.into_inner() {
        let modifier = fov.previous.lerp(fov.current, time.overstep_fraction());
        perspective.fov = (FOV * modifier).to_radians();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mcrs_minecraft_client_world::player::{look_direction, spawn_player};

    #[test]
    fn a_spawned_player_carries_the_camera() {
        let mut app = App::new();
        app.add_plugins(CameraPlugin);
        let player = spawn_player(app.world_mut(), DVec3::new(1.0, 64.0, 2.0), 0.0, 0.0);
        let world = app.world_mut();
        let parent = world
            .query_filtered::<&ChildOf, With<PlayerCamera>>()
            .single(world)
            .ok()
            .map(ChildOf::parent);
        assert_eq!(parent, Some(player));
        assert_eq!(
            world.get::<Transform>(player).map(|t| t.translation),
            Some(Vec3::new(1.0, 64.0, 2.0))
        );
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
            let expected = look_direction(yaw, pitch);
            assert!(
                forward.abs_diff_eq(expected, 1e-5),
                "yaw {yaw}, pitch {pitch}: {forward:?} != {expected:?}"
            );
        }
    }

    #[test]
    fn the_field_of_view_eases_halfway_to_the_sprint_target_each_tick() {
        let mut modifier = FovFilter::default().current;
        for expected in [82.775, 85.6625, 87.10625, 87.828125, 88.18906] {
            modifier = next_fov_modifier(modifier, SPRINTING_FOV_MODIFIER);
            assert!(
                (FOV * modifier - expected).abs() < 1e-4,
                "{}",
                FOV * modifier
            );
        }
    }
}
