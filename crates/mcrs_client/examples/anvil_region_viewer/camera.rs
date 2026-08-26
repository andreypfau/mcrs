use bevy::core_pipeline::tonemapping::Tonemapping;
use bevy::input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll, MouseScrollUnit};
use bevy::prelude::*;
use bevy::render::view::Msaa;

use crate::config;

#[derive(Resource)]
pub struct Sweep(pub f32);

#[derive(Component)]
pub struct Orbit {
    pub yaw: f32,
    pub pitch: f32,
    pub radius: f32,
    pub target: Vec3,
}

pub fn spawn(mut commands: Commands) {
    commands.spawn((
        Camera3d::default(),
        Tonemapping::None,
        Msaa::Off,
        Projection::Perspective(PerspectiveProjection {
            far: 4000.0,
            ..default()
        }),
        Transform::default(),
        config::starting_orbit(),
    ));
}

pub fn orbit(
    buttons: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    motion: Res<AccumulatedMouseMotion>,
    scroll: Res<AccumulatedMouseScroll>,
    time: Res<Time>,
    sweep: Res<Sweep>,
    camera: Single<(&mut Orbit, &mut Transform)>,
) {
    let (mut orbit, mut transform) = camera.into_inner();
    orbit.target.x += sweep.0 * time.delta_secs();
    let panning = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
    if buttons.pressed(MouseButton::Left) {
        if panning {
            let right = transform.right() * -motion.delta.x * orbit.radius * 0.002;
            let up = transform.up() * motion.delta.y * orbit.radius * 0.002;
            orbit.target += right + up;
        } else {
            orbit.yaw -= motion.delta.x * 0.005;
            orbit.pitch = (orbit.pitch - motion.delta.y * 0.005).clamp(-1.54, 1.54);
        }
    }
    let zoom = match scroll.unit {
        MouseScrollUnit::Line => scroll.delta.y,
        MouseScrollUnit::Pixel => scroll.delta.y / MouseScrollUnit::SCROLL_UNIT_CONVERSION_FACTOR,
    };
    orbit.radius = (orbit.radius * (1.0 - zoom * 0.08)).clamp(2.0, 2000.0);

    let rotation = Quat::from_euler(EulerRot::YXZ, orbit.yaw, -orbit.pitch, 0.0);
    let target = orbit.target;
    *transform = Transform::from_translation(target + rotation * (Vec3::Z * orbit.radius))
        .looking_at(target, Vec3::Y);
}
