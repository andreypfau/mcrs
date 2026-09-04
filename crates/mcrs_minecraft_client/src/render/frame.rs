use bevy::math::primitives::ViewFrustum;
use bevy::prelude::*;
use bevy::render::render_resource::*;
use bevy::render::renderer::{RenderDevice, RenderQueue};
use bevy::render::view::ExtractedView;

use super::draws::PARAMS_STRIDE;
use super::stats::args_reset;
use super::terrain::Terrain;
use crate::camera::CameraOrigin;
use crate::mesh::STREAMS;
use mcrs_minecraft_network::columns::SECTION_SIZE;

use super::Budget;

/// The frame's view, expressed against the origin of the section the camera stands in. Nothing
/// here is an absolute world coordinate: at the edge of the world f32 has no block left to give.
#[derive(Copy, Clone, Default, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
pub(super) struct CameraUniform {
    clip_from_relative: [f32; 16],
    frustum: [[f32; 4]; SIDE_PLANES],
    section: [i32; 3],
    _pad_section: i32,
    offset: [f32; 3],
    _pad_offset: f32,
    tint_origin: [f32; 2],
    tint_scale: [f32; 2],
    animated_from: u32,
    hiz_levels: u32,
    _pad: [u32; 2],
}

pub(super) const CAMERA_SIZE: u64 = size_of::<CameraUniform>() as u64;

/// The projection is an infinite reverse-Z one, so the sixth plane a frustum carries is the
/// degenerate far plane and nothing to test against.
const SIDE_PLANES: usize = 5;

pub(super) struct Frame {
    pub params: Buffer,
    pub camera: Buffer,
    pub cave: Buffer,
    pub args: Buffer,
    // Copied over `args` once a frame so the cull pass starts from zeroed instance counts
    // without one small clear per draw.
    pub args_reset: Buffer,
    pub args_readback: Buffer,
}

pub(crate) fn uniform(label: &str, size: u64, device: &RenderDevice) -> Buffer {
    device.create_buffer(&BufferDescriptor {
        label: Some(label),
        size,
        usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

impl Frame {
    pub fn new(budget: &Budget, device: &RenderDevice) -> Self {
        let args_init = args_reset();
        Self {
            params: uniform(
                "terrain draw params",
                STREAMS as u64 * PARAMS_STRIDE as u64,
                device,
            ),
            camera: uniform("terrain camera", CAMERA_SIZE, device),
            cave: device.create_buffer_with_data(&BufferInitDescriptor {
                label: Some("terrain cave visibility"),
                contents: bytemuck::cast_slice(&vec![u32::MAX; budget.sections.div_ceil(32)]),
                usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
            }),
            args: device.create_buffer_with_data(&BufferInitDescriptor {
                label: Some("terrain draw args"),
                contents: bytemuck::cast_slice(&args_init),
                usage: BufferUsages::STORAGE
                    | BufferUsages::INDIRECT
                    | BufferUsages::COPY_DST
                    | BufferUsages::COPY_SRC,
            }),
            args_reset: device.create_buffer_with_data(&BufferInitDescriptor {
                label: Some("terrain draw args reset"),
                contents: bytemuck::cast_slice(&args_init),
                usage: BufferUsages::COPY_SRC,
            }),
            args_readback: device.create_buffer(&BufferDescriptor {
                label: Some("terrain draw args readback"),
                size: size_of_val(args_init.as_slice()) as u64,
                usage: BufferUsages::MAP_READ | BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }),
        }
    }
}

fn clip_from_relative(clip_from_view: Mat4, rotation: Quat, offset: Vec3) -> Mat4 {
    clip_from_view * Mat4::from_rotation_translation(rotation, offset).inverse()
}

pub(super) fn write_camera(
    terrain: Option<Res<Terrain>>,
    origin: Option<Res<CameraOrigin>>,
    views: Query<&ExtractedView, With<Camera3d>>,
    queue: Res<RenderQueue>,
) {
    let (Some(terrain), Some(origin), Some(view)) = (terrain, origin, views.iter().next()) else {
        return;
    };
    let clip_from_relative = clip_from_relative(
        view.clip_from_view,
        view.world_from_view.rotation(),
        origin.offset,
    );
    let frustum = ViewFrustum::from_clip_from_world(&clip_from_relative);
    // The tint window wraps, so only the camera section's place inside it matters, and that
    // stays exact in i32 where the section's absolute position would not in f32.
    let tint_origin = [
        -(origin.section.x * SECTION_SIZE as i32).rem_euclid(terrain.budget.tint_size[0] as i32)
            as f32,
        -(origin.section.z * SECTION_SIZE as i32).rem_euclid(terrain.budget.tint_size[1] as i32)
            as f32,
    ];
    queue.write_buffer(
        &terrain.frame.camera,
        0,
        bytemuck::bytes_of(&CameraUniform {
            clip_from_relative: clip_from_relative.to_cols_array(),
            frustum: std::array::from_fn(|plane| frustum.half_spaces[plane].normal_d().to_array()),
            section: origin.section.to_array(),
            offset: origin.offset.to_array(),
            tint_origin,
            tint_scale: [
                1.0 / terrain.budget.tint_size[0] as f32,
                1.0 / terrain.budget.tint_size[1] as f32,
            ],
            animated_from: terrain.sprites.animated_from,
            hiz_levels: terrain.hiz.levels(),
            ..default()
        }),
    );
}

#[cfg(test)]
mod tests {
    use bevy::math::{DMat4, DVec3};

    use super::*;

    fn ndc(clip: Vec4) -> Vec2 {
        clip.xy() / clip.w
    }

    /// A block a render distance away, seen from the far edge of the world. Against an f64
    /// ground truth the camera-relative projection holds to a ten-thousandth of a pixel, while
    /// the same projection built on absolute f32 coordinates drifts by more than one.
    #[test]
    fn the_relative_projection_survives_a_coordinate_f32_cannot_hold() {
        let eye = DVec3::new(30_000_000.5, 16_777_216.25, -30_000_000.5);
        let origin = CameraOrigin::of(eye);
        let block = (origin.section + IVec3::new(96, -4, -96)) * SECTION_SIZE as i32;
        let rotation =
            Quat::from_rotation_arc(Vec3::NEG_Z, (block.as_dvec3() - eye).normalize().as_vec3())
                * Quat::from_rotation_y(0.3);
        let clip_from_view = Mat4::perspective_infinite_reverse_rh(1.4, 16.0 / 9.0, 0.05);

        let truth = ndc((clip_from_view.as_dmat4()
            * DMat4::from_rotation_translation(rotation.as_dquat(), eye).inverse()
            * block.as_dvec3().extend(1.0))
        .as_vec4());
        let relative = ndc(clip_from_relative(clip_from_view, rotation, origin.offset)
            * origin.relative(block).extend(1.0));
        let absolute = ndc(clip_from_view
            * Mat4::from_rotation_translation(rotation, eye.as_vec3()).inverse()
            * block.as_vec3().extend(1.0));

        assert!(relative.distance(truth) < 1e-6, "{relative} {truth}");
        assert!(absolute.distance(truth) > 1e-3, "{absolute} {truth}");
    }
}
