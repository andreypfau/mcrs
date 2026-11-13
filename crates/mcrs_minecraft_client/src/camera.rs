use bevy::camera::CameraUpdateSystems;
use bevy::core_pipeline::tonemapping::Tonemapping;
use bevy::math::DVec3;
use bevy::prelude::*;
use bevy::render::render_resource::TextureUsages;
use bevy::render::view::Msaa;
use mcrs_minecraft_client_options::ClientOptions;
use mcrs_minecraft_client_world::local_player::{Flying, LocalPlayerTick, Sprint};
use mcrs_minecraft_client_world::player::{Player, look_direction};
use mcrs_minecraft_client_world::player_view::{ChunkCacheRadius, RequestedViewDistance};
use mcrs_minecraft_engine_world::entity::physics::{
    OldTransform, Rotation, Transform as PhysicsTransform,
};
use mcrs_minecraft_render::CameraOrigin;

use crate::config::VIEW_DISTANCE;

const STANDING_EYE_HEIGHT: f32 = 1.62;
const DEFAULT_WALKING_SPEED: f32 = 0.1;
const BASE_MOVEMENT_SPEED: f32 = 0.1;
const SPRINT_SPEED_MODIFIER: f32 = 0.3;
const MIN_MOVEMENT_SPEED: f64 = 0.0;
const MAX_MOVEMENT_SPEED: f64 = 1024.0;
const FLYING_MODIFIER: f32 = 1.1;
const MIN_ZOOM: f32 = 0.1;
const MAX_ZOOM: f32 = 1.5;
const NEAR_PLANE: f32 = 0.05;
const BLOCKS_PER_CHUNK_OF_RENDER_DISTANCE: f32 = 64.0;
/// Chunks; mcrs has no option for the cloud range, so it keeps the default.
const CLOUD_RANGE: u8 = 128;
const BLOCKS_PER_CHUNK_OF_CLOUD_RANGE: f32 = 16.0;

/// Places the first-person camera of the local player in each frame, filters its eye height and
/// zoom in each tick, and keeps its far plane at the render distance. Without a
/// [`RequestedViewDistance`] the far plane stays at that of the default render distance.
pub struct CameraPlugin;

impl Plugin for CameraPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<FovOptions>()
            .register_required_components::<Player, Transform>()
            .add_systems(Startup, spawn_camera)
            .add_systems(FixedUpdate, tick_camera.before(LocalPlayerTick))
            .add_systems(
                PostUpdate,
                (
                    place_camera,
                    follow_render_distance.run_if(
                        resource_exists_and_changed::<RequestedViewDistance>
                            .or_else(any_match_filter::<Changed<ChunkCacheRadius>>),
                    ),
                )
                    .chain()
                    .before(TransformSystems::Propagate)
                    .before(CameraUpdateSystems),
            );
    }
}

/// The world camera, at the eye of the local player.
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct PlayerCamera;

/// The field of view that the user selects, and the scale of the changes that the state of the
/// player makes to it.
#[derive(Resource, Debug, Clone, Copy, PartialEq)]
pub struct FovOptions {
    /// Degrees, in `[30, 110]`.
    pub fov: u8,
    /// In `[0, 1]`.
    pub effect_scale: f32,
}

impl Default for FovOptions {
    fn default() -> Self {
        let options = ClientOptions::default();
        Self {
            fov: options.fov,
            effect_scale: options.fov_effect_scale,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct Smoothed {
    current: f32,
    previous: f32,
}

impl Smoothed {
    fn settled(value: f32) -> Self {
        Self {
            current: value,
            previous: value,
        }
    }

    fn approach(&mut self, target: f32) {
        self.previous = self.current;
        self.current += (target - self.current) * 0.5;
    }

    #[must_use]
    fn at(self, partial_tick: f32) -> f32 {
        self.previous + (self.current - self.previous) * partial_tick
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct ViewFilters {
    eye: Smoothed,
    zoom: Smoothed,
}

impl ViewFilters {
    fn tick(&mut self, eye_height: f32, target_zoom: f32) {
        self.eye.approach(eye_height);
        self.zoom.approach(target_zoom);
        self.zoom.current = self.zoom.current.clamp(MIN_ZOOM, MAX_ZOOM);
    }
}

/// Empty until the first tick or frame that sees a local player, which starts both filters
/// settled so the first frame of a world is already at the eye and at the target zoom.
#[derive(Component, Debug, Default)]
struct CameraFilters(Option<ViewFilters>);

impl CameraFilters {
    fn get_or_settle(&mut self, eye_height: f32, target_zoom: f32) -> &mut ViewFilters {
        self.0.get_or_insert(ViewFilters {
            eye: Smoothed::settled(eye_height),
            zoom: Smoothed::settled(target_zoom),
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct Motion {
    flying: bool,
    sprinting: bool,
}

#[must_use]
fn movement_speed(sprinting: bool) -> f32 {
    let mut speed = f64::from(BASE_MOVEMENT_SPEED);
    if sprinting {
        speed *= 1.0 + f64::from(SPRINT_SPEED_MODIFIER);
    }
    speed.clamp(MIN_MOVEMENT_SPEED, MAX_MOVEMENT_SPEED) as f32
}

#[must_use]
fn target_zoom(motion: Motion, walking_speed: f32, effect_scale: f32) -> f32 {
    let mut modifier = 1.0;
    if motion.flying {
        modifier *= FLYING_MODIFIER;
    }
    if walking_speed != 0.0 {
        modifier *= (movement_speed(motion.sprinting) / walking_speed + 1.0) / 2.0;
    }
    1.0 + (modifier - 1.0) * effect_scale
}

/// Vertical, in degrees.
#[must_use]
fn field_of_view(options: FovOptions, zoom: Smoothed, partial_tick: f32) -> f32 {
    f32::from(options.fov) * zoom.at(partial_tick)
}

#[must_use]
fn orientation(yaw: f32, pitch: f32) -> Quat {
    Transform::IDENTITY
        .looking_to(
            look_direction(yaw, pitch),
            look_direction(yaw, pitch - 90.0),
        )
        .rotation
}

/// The stored yaws are wrapped, so the turn between them is taken the short way round.
#[must_use]
fn frame_rotation(previous: Rotation, current: Rotation, partial_tick: f32) -> Quat {
    let turn = (current.yaw() - previous.yaw() + 180.0).rem_euclid(360.0) - 180.0;
    let yaw = previous.yaw() + turn * partial_tick;
    let pitch = previous.pitch() + (current.pitch() - previous.pitch()) * partial_tick;
    orientation(yaw, pitch)
}

/// The distance the options ask for, capped by the server radius once that is above 0.
// chisle: only the far plane follows it; each held column is drawn and there is no
// render-distance fog (MENU-DIST-001 steps 3, 5 to 7). A draw test and a fog that read it
// lift this.
#[must_use]
fn render_distance(requested: u8, server_radius: i32) -> u8 {
    match u8::try_from(server_radius) {
        Ok(radius) if radius > 0 => requested.min(radius),
        _ => requested,
    }
}

#[must_use]
fn far_plane(render_distance: u8) -> f32 {
    (f32::from(render_distance) * BLOCKS_PER_CHUNK_OF_RENDER_DISTANCE)
        .max(f32::from(CLOUD_RANGE) * BLOCKS_PER_CHUNK_OF_CLOUD_RANGE)
}

fn spawn_camera(mut commands: Commands) {
    commands.spawn((
        PlayerCamera,
        CameraFilters::default(),
        Camera3d {
            // The cull reads the last frame's depth back as a pyramid.
            depth_texture_usages: (TextureUsages::RENDER_ATTACHMENT
                | TextureUsages::TEXTURE_BINDING)
                .into(),
            ..default()
        },
        // The sky pipelines are built for a single sample; multisampling the view would leave
        // them unable to render into it.
        Msaa::Off,
        // The default tonemapper needs the `tonemapping_luts` feature, which nothing else here
        // requires.
        Tonemapping::None,
        Projection::Perspective(PerspectiveProjection {
            fov: f32::from(FovOptions::default().fov).to_radians(),
            near: NEAR_PLANE,
            near_clip_plane: Vec4::new(0.0, 0.0, -1.0, -NEAR_PLANE),
            far: far_plane(VIEW_DISTANCE),
            ..default()
        }),
    ));
}

fn follow_render_distance(
    requested: Option<Res<RequestedViewDistance>>,
    server: Option<Single<&ChunkCacheRadius>>,
    mut projection: Single<&mut Projection, With<PlayerCamera>>,
) {
    let Some(requested) = requested else {
        return;
    };
    let far = far_plane(render_distance(
        requested.0,
        server.map_or(0, |radius| radius.0),
    ));
    if let Projection::Perspective(perspective) = projection.as_ref()
        && perspective.far != far
        && let Projection::Perspective(perspective) = projection.as_mut()
    {
        perspective.far = far;
    }
}

fn tick_camera(
    options: Res<FovOptions>,
    player: Single<(Has<Flying>, &Sprint), With<Player>>,
    mut filters: Single<&mut CameraFilters, With<PlayerCamera>>,
) {
    let (flying, sprint) = *player;
    let motion = Motion {
        flying,
        sprinting: sprint.active,
    };
    let zoom = target_zoom(motion, DEFAULT_WALKING_SPEED, options.effect_scale);
    filters
        .get_or_settle(STANDING_EYE_HEIGHT, zoom)
        .tick(STANDING_EYE_HEIGHT, zoom);
}

fn place_camera(
    time: Res<Time<Fixed>>,
    options: Res<FovOptions>,
    player: Single<
        (
            &PhysicsTransform,
            &OldTransform,
            Has<Flying>,
            &Sprint,
            &mut Transform,
        ),
        With<Player>,
    >,
    camera: Single<
        (&mut CameraFilters, &mut Transform, &mut Projection),
        (With<PlayerCamera>, Without<Player>),
    >,
    origin: Option<ResMut<CameraOrigin>>,
) {
    let (current, previous, flying, sprint, mut body) = player.into_inner();
    let (mut filters, mut camera, mut projection) = camera.into_inner();
    let motion = Motion {
        flying,
        sprinting: sprint.active,
    };
    let filters = *filters.get_or_settle(
        STANDING_EYE_HEIGHT,
        target_zoom(motion, DEFAULT_WALKING_SPEED, options.effect_scale),
    );
    let partial_tick = time.overstep_fraction();

    let frame_position = previous
        .translation
        .lerp(current.translation, time.overstep_fraction_f64());
    let eye = frame_position + DVec3::Y * f64::from(filters.eye.at(partial_tick));
    let placed = Transform {
        translation: eye.as_vec3(),
        rotation: frame_rotation(previous.rotation, current.rotation, partial_tick),
        ..Transform::IDENTITY
    };
    camera.set_if_neq(placed);
    body.set_if_neq(Transform::from_translation(frame_position.as_vec3()));
    if let Some(mut origin) = origin {
        origin.set_if_neq(CameraOrigin::of(eye));
    }

    let fov = field_of_view(*options, filters.zoom, partial_tick).to_radians();
    if let Projection::Perspective(perspective) = projection.as_ref()
        && perspective.fov != fov
        && let Projection::Perspective(perspective) = projection.as_mut()
    {
        perspective.fov = fov;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::options::ClientOptionsPlugin;

    fn camera_far(app: &mut App) -> f32 {
        let mut cameras = app
            .world_mut()
            .query_filtered::<&Projection, With<PlayerCamera>>();
        match cameras.single(app.world()).unwrap() {
            Projection::Perspective(perspective) => perspective.far,
            other => panic!("the player camera has a perspective projection, not {other:?}"),
        }
    }

    #[test]
    fn the_far_plane_follows_the_option_capped_by_the_server_radius_from_the_next_frame() {
        enum Change {
            Requested(u8),
            ServerRadius(i32),
        }
        let mut options = ClientOptions::default();
        options.view_distance = Some(12);
        let mut app = App::new();
        app.init_resource::<Time<Fixed>>().add_plugins((
            ClientOptionsPlugin {
                options,
                file: None,
                gui_scale: None,
            },
            CameraPlugin,
        ));
        let connection = app.world_mut().spawn(ChunkCacheRadius(10)).id();
        app.update();
        assert_eq!(
            camera_far(&mut app),
            2048.0,
            "o = 12 with v = 10 gives r = 10, inside the cloud range"
        );

        for (change, far) in [
            (Change::Requested(96), 2048.0),
            (Change::ServerRadius(0), 6144.0),
            (Change::Requested(40), 2560.0),
            (Change::ServerRadius(36), 2304.0),
            (Change::Requested(2), 2048.0),
        ] {
            match change {
                Change::Requested(columns) => {
                    app.world_mut()
                        .resource_mut::<ClientOptions>()
                        .view_distance = Some(columns);
                }
                Change::ServerRadius(radius) => {
                    app.world_mut()
                        .entity_mut(connection)
                        .insert(ChunkCacheRadius(radius));
                }
            }
            app.update();
            assert_eq!(camera_far(&mut app), far);
        }
    }

    #[test]
    fn without_client_options_the_far_plane_keeps_that_of_the_spawn() {
        let mut app = App::new();
        app.init_resource::<Time<Fixed>>().add_plugins(CameraPlugin);
        app.world_mut().spawn(ChunkCacheRadius(4));
        app.update();
        assert_eq!(
            camera_far(&mut app),
            6144.0,
            "the default render distance is 96"
        );
    }

    #[test]
    fn the_zoom_of_a_player_that_starts_to_sprint_while_flying_halves_its_way_to_the_target() {
        let options = FovOptions::default();
        let flying = Motion {
            flying: true,
            sprinting: false,
        };
        let sprinting = Motion {
            flying: true,
            sprinting: true,
        };
        let walking_speed = DEFAULT_WALKING_SPEED;
        let start = target_zoom(flying, walking_speed, options.effect_scale);
        assert!((start - 1.1).abs() < 1e-6, "flying alone is {start}");
        let target = target_zoom(sprinting, walking_speed, options.effect_scale);
        let mut filters = ViewFilters {
            eye: Smoothed::settled(STANDING_EYE_HEIGHT),
            zoom: Smoothed::settled(start),
        };
        for expected in [82.775, 85.6625, 87.10625, 87.828125, 88.18906] {
            filters.tick(STANDING_EYE_HEIGHT, target);
            let fov = f32::from(options.fov) * filters.zoom.current;
            assert!((fov - expected).abs() < 1e-4, "{fov} != {expected}");
        }
        let between = field_of_view(options, filters.zoom, 0.25);
        assert!(
            (between - 87.918_36).abs() < 1e-3,
            "a quarter of the way from 87.828125 to 88.18906 is {between}"
        );
    }

    #[test]
    fn the_camera_looks_along_the_facing_vector() {
        for (yaw, pitch) in [
            (0.0, 0.0),
            (-139.9493, 8.099984),
            (90.0, -30.0),
            (180.0, 45.0),
            (-90.0, 90.0),
            (45.0, -90.0),
        ] {
            let rotation = Rotation::new(yaw, pitch);
            let view = frame_rotation(rotation, rotation, 0.5) * Vec3::NEG_Z;
            let facing = look_direction(yaw, pitch);
            assert!(
                view.abs_diff_eq(facing, 1e-5),
                "({yaw}, {pitch}): {view:?} != {facing:?}"
            );
        }
    }
}
