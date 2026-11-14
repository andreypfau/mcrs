use bevy::math::DVec3;
use bevy::prelude::*;
use bevy::render::RenderApp;
use bevy::render::extract_resource::ExtractResource;
use bevy::render::render_resource::*;
use bevy::render::renderer::{RenderDevice, RenderQueue};
use bevy::render::view::ExtractedView;

use super::Occlusion;
use super::WorldFog;
use super::draws::PARAMS_STRIDE;
use super::frame_view::{FrameView, SIDE_PLANES};
use super::terrain::Terrain;
use mcrs_minecraft_core::CardinalLight;
use mcrs_minecraft_mesh::face_shade;
use mcrs_minecraft_mesh::{SECTION_SIZE, STREAMS};

/// The camera split into the section it stands in and where it stands inside that section, so
/// the terrain shaders never need an absolute world coordinate in f32.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, ExtractResource)]
#[extract_app(RenderApp)]
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

/// The directional shade table of the dimension the camera is in.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq, ExtractResource)]
#[extract_app(RenderApp)]
pub struct TerrainShade(pub CardinalLight);

impl TerrainShade {
    #[must_use]
    pub fn is_nether(self) -> bool {
        self.0 == CardinalLight::Nether
    }
}

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
    hiz_levels: u32,
    quad_cull: u32,
    viewport: [f32; 2],
    shade_table: u32,
    render_distance: u32,
    _pad_render_distance: [u32; 2],
    fog_color: [f32; 3],
    _pad_fog_color: f32,
    fog_environmental: [f32; 2],
    fog_render_distance: [f32; 2],
}

pub(super) const CAMERA_SIZE: u64 = size_of::<CameraUniform>() as u64;

pub(super) struct Frame {
    pub params: Buffer,
    pub camera: Buffer,
}

pub fn uniform(label: &str, size: u64, device: &RenderDevice) -> Buffer {
    device.create_buffer(&BufferDescriptor {
        label: Some(label),
        size,
        usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

impl Frame {
    pub fn new(device: &RenderDevice) -> Self {
        Self {
            params: uniform(
                "terrain draw params",
                STREAMS as u64 * PARAMS_STRIDE as u64,
                device,
            ),
            camera: uniform("terrain camera", CAMERA_SIZE, device),
        }
    }
}

pub fn clip_from_relative(clip_from_view: Mat4, rotation: Quat, offset: Vec3) -> Mat4 {
    clip_from_view * Mat4::from_rotation_translation(rotation, offset).inverse()
}

/// Publishes the view of the frame for the passes of the render world.
pub(super) fn write_frame_view(
    mut commands: Commands,
    origin: Option<Res<CameraOrigin>>,
    views: Query<&ExtractedView, With<Camera3d>>,
) {
    let (Some(origin), Some(view)) = (origin, views.iter().next()) else {
        return;
    };
    commands.insert_resource(FrameView::new(
        view.clip_from_view,
        Mat4::from_rotation_translation(view.world_from_view.rotation(), origin.offset).inverse(),
        *origin,
        view.viewport.zw().as_vec2(),
    ));
}

#[expect(
    clippy::too_many_arguments,
    reason = "a Bevy render system: each resource is its own parameter so the scheduler sees the access"
)]
pub(super) fn write_camera(
    terrain: Option<Res<Terrain>>,
    frame_view: Option<Res<FrameView>>,
    occlusion: Res<Occlusion>,
    quad_cull: Res<super::QuadCull>,
    render_distance: Res<super::RenderDistance>,
    shade: Res<TerrainShade>,
    fog: Res<WorldFog>,
    queue: Res<RenderQueue>,
) {
    let (Some(terrain), Some(frame_view)) = (terrain, frame_view) else {
        return;
    };
    let section = frame_view.camera_section;
    // The tint window wraps, so only the camera section's place inside it matters, and that
    // stays exact in i32 where the section's absolute position would not in f32.
    let tint_origin = [
        -(section.x * SECTION_SIZE as i32).rem_euclid(terrain.budget.tint_size[0] as i32) as f32,
        -(section.z * SECTION_SIZE as i32).rem_euclid(terrain.budget.tint_size[1] as i32) as f32,
    ];
    queue.write_buffer(
        &terrain.frame.camera,
        0,
        bytemuck::bytes_of(&CameraUniform {
            clip_from_relative: frame_view.clip_from_relative.to_cols_array(),
            frustum: frame_view.frustum.map(|plane| plane.to_array()),
            section: section.to_array(),
            offset: frame_view.camera_offset.to_array(),
            tint_origin,
            tint_scale: [
                1.0 / terrain.budget.tint_size[0] as f32,
                1.0 / terrain.budget.tint_size[1] as f32,
            ],
            hiz_levels: terrain.hiz_levels(&occlusion),
            viewport: frame_view.viewport.to_array(),
            quad_cull: quad_cull.0 as u32,
            shade_table: face_shade::shader_index(shade.0),
            // chisle: only the section cull of the full path reads it; the minimal path draws
            // each held column. A test of the section in its vertex stage lifts this.
            render_distance: u32::from(render_distance.0),
            fog_color: fog.color,
            fog_environmental: [fog.environmental.start, fog.environmental.end],
            fog_render_distance: [fog.render_distance.start, fog.render_distance.end],
            ..default()
        }),
    );
}

#[cfg(test)]
mod tests {
    use std::mem;

    use bevy::math::proj;
    use bevy::math::{DMat4, DVec3};

    use super::*;

    fn ndc(clip: Vec4) -> Vec2 {
        clip.xy() / clip.w
    }

    #[test]
    fn the_camera_uniform_has_the_layout_of_the_shader_camera() {
        let module =
            naga::front::wgsl::parse_str(include_str!("shaders/include/frame.wesl")).unwrap();
        let Some(naga::TypeInner::Struct { members, span }) = module
            .types
            .iter()
            .find(|(_, ty)| ty.name.as_deref() == Some("Camera"))
            .map(|(_, ty)| &ty.inner)
        else {
            panic!("frame.wesl declares the struct Camera");
        };
        let rust = [
            (
                "clip_from_relative",
                mem::offset_of!(CameraUniform, clip_from_relative),
            ),
            ("frustum", mem::offset_of!(CameraUniform, frustum)),
            ("section", mem::offset_of!(CameraUniform, section)),
            ("offset", mem::offset_of!(CameraUniform, offset)),
            ("tint_origin", mem::offset_of!(CameraUniform, tint_origin)),
            ("tint_scale", mem::offset_of!(CameraUniform, tint_scale)),
            ("hiz_levels", mem::offset_of!(CameraUniform, hiz_levels)),
            ("quad_cull", mem::offset_of!(CameraUniform, quad_cull)),
            ("viewport", mem::offset_of!(CameraUniform, viewport)),
            ("shade_table", mem::offset_of!(CameraUniform, shade_table)),
            (
                "render_distance",
                mem::offset_of!(CameraUniform, render_distance),
            ),
            ("fog_color", mem::offset_of!(CameraUniform, fog_color)),
            (
                "fog_environmental",
                mem::offset_of!(CameraUniform, fog_environmental),
            ),
            (
                "fog_render_distance",
                mem::offset_of!(CameraUniform, fog_render_distance),
            ),
        ];
        let shader: Vec<(&str, usize)> = members
            .iter()
            .map(|member| (member.name.as_deref().unwrap_or(""), member.offset as usize))
            .collect();
        assert_eq!(shader, rust);
        assert_eq!(*span as usize, size_of::<CameraUniform>());
    }

    #[test]
    fn a_far_section_keeps_the_precision_an_absolute_f32_has_lost() {
        let eye = DVec3::new(30_000_000.5, 16_777_216.25, -30_000_000.5);
        let origin = CameraOrigin::of(eye);
        let far = (origin.section + IVec3::new(96, 0, -96)) * SECTION_SIZE as i32;
        let truth = (far.as_dvec3() - eye).as_vec3();

        assert_eq!(origin.relative(far) - origin.offset, truth);
        assert_ne!(far.as_vec3() - eye.as_vec3(), truth);
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
        let clip_from_view = proj::perspective_infinite_reverse(1.4, 16.0 / 9.0, 0.05);

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
