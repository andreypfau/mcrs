use bevy::math::DVec3;
use bevy::prelude::*;
use bevy::render::extract_resource::{ExtractResource, ExtractResourcePlugin};

use mcrs_voxel_world::entity::physics::{OldTransform, Transform as PhysicsTransform};

use mcrs_minecraft_network::columns::SECTION_SIZE;
use crate::local_player::{LocalPlayerTick, Sprint};
use crate::options::FOV;
use crate::player::{EYE_HEIGHT, Player, PlayerCamera};

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

/// The camera split into the section it stands in and where it stands inside that section, so
/// the terrain shaders never need an absolute world coordinate in f32.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, ExtractResource)]
pub struct CameraOrigin {
    pub section: IVec3,
    pub offset: Vec3,
}

impl CameraOrigin {
    pub fn of(eye: DVec3) -> Self {
        let section = (eye / SECTION_SIZE as f64).floor();
        Self {
            section: section.as_ivec3(),
            offset: (eye - section * SECTION_SIZE as f64).as_vec3(),
        }
    }

    /// Subtracted in i32, so a block at the edge of the world still lands where it belongs.
    pub fn relative(&self, block: IVec3) -> Vec3 {
        (block - self.section * SECTION_SIZE as i32).as_vec3()
    }
}

pub struct CameraPlugin;

impl Plugin for CameraPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CameraOrigin>()
            .add_plugins(ExtractResourcePlugin::<CameraOrigin>::default())
            .add_systems(FixedUpdate, tick_fov.after(LocalPlayerTick))
            .add_systems(Update, (interpolate_render_position, apply_fov));
    }
}

fn next_fov_modifier(current: f32, target: f32) -> f32 {
    (current + (target - current) * FOV_FILTER_RATE).clamp(FOV_MODIFIER_MIN, FOV_MODIFIER_MAX)
}

/// `Camera.tickFov`.
fn tick_fov(sprint: Single<&Sprint, With<Player>>, mut fov: Single<&mut FovFilter>) {
    let target = if sprint.active {
        SPRINTING_FOV_MODIFIER
    } else {
        FLYING_FOV_MODIFIER
    };
    fov.previous = fov.current;
    fov.current = next_fov_modifier(fov.current, target);
}

/// `Camera.alignWithEntity`.
fn interpolate_render_position(
    time: Res<Time<Fixed>>,
    player: Single<(&PhysicsTransform, &OldTransform, &mut Transform), With<Player>>,
    mut origin: ResMut<CameraOrigin>,
) {
    let (physics, old_physics, mut transform) = player.into_inner();
    let translation = old_physics
        .translation
        .lerp(physics.translation, time.overstep_fraction_f64());
    transform.translation = translation.as_vec3();
    *origin = CameraOrigin::of(translation + DVec3::Y * EYE_HEIGHT as f64);
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

    #[test]
    fn a_far_section_keeps_the_precision_an_absolute_f32_has_lost() {
        let eye = DVec3::new(30_000_000.5, 16_777_216.25, -30_000_000.5);
        let origin = CameraOrigin::of(eye);
        let far = (origin.section + IVec3::new(96, 0, -96)) * SECTION_SIZE as i32;
        let truth = (far.as_dvec3() - eye).as_vec3();

        assert_eq!(origin.relative(far) - origin.offset, truth);
        assert_ne!(far.as_vec3() - eye.as_vec3(), truth);
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
