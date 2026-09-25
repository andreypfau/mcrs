use bevy::prelude::*;
use bevy::render::renderer::RenderQueue;
use bevy::render::view::ExtractedView;
use mcrs_minecraft_render::{CameraOrigin, Raster, clip_from_relative};

use crate::gbuffer::DeferredFrame;

/// Turns a G-buffer pixel back into a position against the origin of the camera's section, the
/// same space terrain is drawn in, so no absolute world coordinate is ever held in f32.
#[derive(Copy, Clone, Default, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
pub(crate) struct LightingUniform {
    relative_from_clip: [f32; 16],
    viewport: [f32; 2],
    _pad: [f32; 2],
}

pub(crate) const LIGHTING_UNIFORM_SIZE: u64 = size_of::<LightingUniform>() as u64;

pub(crate) fn write_lighting_uniform(
    frame: Option<Res<DeferredFrame>>,
    origin: Option<Res<CameraOrigin>>,
    raster: Res<Raster>,
    views: Query<&ExtractedView, With<Camera3d>>,
    queue: Res<RenderQueue>,
) {
    let (Some(frame), Some(origin), Some(view)) = (frame, origin, views.iter().next()) else {
        return;
    };
    let clip_from_relative = clip_from_relative(
        view.clip_from_view,
        view.world_from_view.rotation(),
        origin.offset,
    );
    let viewport = (view.viewport.zw().as_vec2() * raster.0.min(1.0)).max(Vec2::ONE);
    queue.write_buffer(
        &frame.lighting,
        0,
        bytemuck::bytes_of(&LightingUniform {
            relative_from_clip: clip_from_relative.inverse().to_cols_array(),
            viewport: viewport.to_array(),
            ..default()
        }),
    );
}

/// What a fullscreen fragment computes for a pixel, mirrored on the CPU for testing.
#[cfg(test)]
fn reconstruct(relative_from_clip: Mat4, pixel: Vec2, viewport: Vec2, depth: f32) -> Vec3 {
    let ndc = (pixel + 0.5) / viewport * Vec2::new(2.0, -2.0) + Vec2::new(-1.0, 1.0);
    relative_from_clip.project_point3(ndc.extend(depth))
}

#[cfg(test)]
mod tests {
    use bevy::math::DVec3;
    use mcrs_minecraft_mesh::SECTION_SIZE;

    use super::*;

    const VIEWPORT: Vec2 = Vec2::new(1280.0, 720.0);

    /// The continuous pixel a projected point lands on, in the coordinates the fragment stage
    /// numbers pixels by.
    fn pixel_and_depth(clip_from: Mat4, point: Vec3) -> (Vec2, f32) {
        let ndc = clip_from.project_point3(point);
        let pixel = (ndc.xy() * Vec2::new(0.5, -0.5) + 0.5) * VIEWPORT - 0.5;
        (pixel, ndc.z)
    }

    #[test]
    fn a_reconstructed_block_at_the_world_edge_is_the_block_drawn() {
        let eye = DVec3::new(30_000_000.5, 16_777_216.25, -30_000_000.5);
        let origin = CameraOrigin::of(eye);
        let clip_from_view = Mat4::perspective_infinite_reverse_rh(1.4, 16.0 / 9.0, 0.05);
        let inside = DVec3::new(0.3, 0.7, 0.6);
        let eye_block = eye.floor().as_ivec3();
        let sections = 96 * SECTION_SIZE as i32;
        for step in [
            IVec3::new(1, -2, 0),
            IVec3::new(-13, -9, 11),
            IVec3::new(sections + 1, -40, -sections),
        ] {
            let block = eye_block + step;
            let point = block.as_dvec3() + inside;
            let rotation =
                Quat::from_rotation_arc(Vec3::NEG_Z, (point - eye).normalize().as_vec3())
                    * Quat::from_rotation_y(0.3);
            let clip_from = clip_from_relative(clip_from_view, rotation, origin.offset);
            let truth = (point - (origin.section * SECTION_SIZE as i32).as_dvec3()).as_vec3();

            let (pixel, depth) = pixel_and_depth(clip_from, truth);
            assert!(
                pixel.cmpge(Vec2::ZERO).all() && pixel.cmplt(VIEWPORT).all(),
                "{pixel}"
            );
            let relative = reconstruct(clip_from.inverse(), pixel, VIEWPORT, depth);
            let voxel = origin.section * SECTION_SIZE as i32 + relative.floor().as_ivec3();
            assert!(
                relative.distance(truth) < 1e-3,
                "{step}: {relative} {truth}"
            );
            assert_eq!(voxel, block, "{step}");

            let clip_from_world =
                clip_from_view * Mat4::from_rotation_translation(rotation, eye.as_vec3()).inverse();
            let (pixel, depth) = pixel_and_depth(clip_from_world, point.as_vec3());
            let absolute = reconstruct(clip_from_world.inverse(), pixel, VIEWPORT, depth);
            assert_ne!(absolute.floor().as_ivec3(), block, "{step}");
        }
    }
}
