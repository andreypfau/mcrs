use bevy::math::DVec3;
use bevy::prelude::*;
use bevy::render::RenderApp;
use bevy::render::camera::ExtractedCamera;
use bevy::render::extract_resource::ExtractResource;
use bevy::render::render_resource::*;
use bevy::render::renderer::{RenderDevice, RenderQueue};
use bevy::render::view::ExtractedView;

use super::Occlusion;
use super::WorldFog;
use super::draws::PARAMS_STRIDE;
use super::frame_view::FrameView;
use super::shared_types::{CAMERA_SIZE, CameraUniform};
use super::terrain::Terrain;
use mcrs_minecraft_core::CardinalLight;
use mcrs_minecraft_mesh::face_shade;
use mcrs_minecraft_mesh::{DrawKind, SECTION_SIZE};

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

    pub fn relative(&self, block: IVec3) -> Vec3 {
        (block - self.section * SECTION_SIZE as i32).as_vec3()
    }
}

#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq, ExtractResource)]
#[extract_app(RenderApp)]
pub struct TerrainShade(pub CardinalLight);

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
                DrawKind::COUNT as u64 * PARAMS_STRIDE as u64,
                device,
            ),
            camera: uniform("terrain camera", CAMERA_SIZE, device),
        }
    }
}

pub fn clip_from_relative(clip_from_view: Mat4, rotation: Quat, offset: Vec3) -> Mat4 {
    clip_from_view * Mat4::from_rotation_translation(rotation, offset).inverse()
}

pub(super) fn write_frame_view(
    mut commands: Commands,
    origin: Option<Res<CameraOrigin>>,
    frame_view: Option<ResMut<FrameView>>,
    views: Populated<(&ExtractedView, &ExtractedCamera), With<Camera3d>>,
) {
    let (Some(origin), Some((view, _))) = (origin, views.iter().next()) else {
        return;
    };
    let next = FrameView::new(
        view.clip_from_view,
        Mat4::from_rotation_translation(view.world_from_view.rotation(), origin.offset).inverse(),
        *origin,
        view.viewport.zw().as_vec2(),
    );
    match frame_view {
        Some(mut frame_view) => {
            frame_view.set_if_neq(next);
        }
        None => commands.insert_resource(next),
    }
}

pub(super) fn write_camera(
    If(terrain): If<Res<Terrain>>,
    frame_view: Option<Res<FrameView>>,
    views: Query<(&ExtractedView, &ExtractedCamera), With<Camera3d>>,
    occlusion: Res<Occlusion>,
    quad_cull: Res<super::QuadCull>,
    render_distance: Res<super::RenderDistance>,
    shade: Res<TerrainShade>,
    fog: Res<WorldFog>,
    queue: Res<RenderQueue>,
) {
    let Some(frame_view) = frame_view else {
        return;
    };
    let section = frame_view.camera_section;
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
            raster_viewport: views
                .iter()
                .next()
                .and_then(|(_, camera)| camera.physical_target_size)
                .map_or(frame_view.viewport.to_array(), |size| {
                    size.as_vec2().to_array()
                }),
            quad_cull: quad_cull.0 as u32,
            shade_table: face_shade::shader_index(shade.0),
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
    use bevy::math::proj;
    use bevy::math::{DMat4, DVec3};

    use super::*;

    fn ndc(clip: Vec4) -> Vec2 {
        clip.xy() / clip.w
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

    #[test]
    fn the_relative_projection_survives_a_coordinate_f32_cannot_hold() {
        let eye = DVec3::new(30_000_000.9, 16_777_216.9, -30_000_000.9);
        let origin = CameraOrigin::of(eye);
        let block = (origin.section + IVec3::new(48, -4, -48)) * SECTION_SIZE as i32;
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
