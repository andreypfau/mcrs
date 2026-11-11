use bevy::prelude::*;
use bevy::shape::ViewFrustum;

use crate::frame::CameraOrigin;

/// The projection is an infinite reverse-Z one, so the sixth plane a frustum carries is the
/// degenerate far plane and nothing to test against.
pub const SIDE_PLANES: usize = 5;

/// The view of a frame against the origin of the section the camera stands in, which the terrain
/// cull and the entity passes both read. The render world holds it once the terrain has a camera.
#[derive(Resource, Clone, Copy, Debug, PartialEq)]
pub struct FrameView {
    pub clip_from_relative: Mat4,
    /// The left, right, bottom and top planes, then the `w + z` plane of the projection, which
    /// lies behind the eye and is looser than the near plane. Each is `(normal, d)` with the
    /// inside where `normal · p + d > 0`.
    pub frustum: [Vec4; SIDE_PLANES],
    pub camera_section: IVec3,
    /// The eye relative to the origin of the camera section.
    pub camera_offset: Vec3,
    pub viewport: Vec2,
    /// The element `[1][1]` of the projection: `1 / tan(fov_y / 2)`.
    pub focal_y: f32,
}

impl FrameView {
    /// The view of a camera at `origin` with the projection `clip_from_view`, where
    /// `view_from_relative` maps a point relative to the origin of the camera section into view
    /// space.
    #[must_use]
    pub fn new(
        clip_from_view: Mat4,
        view_from_relative: Mat4,
        origin: CameraOrigin,
        viewport: Vec2,
    ) -> Self {
        let clip_from_relative = clip_from_view * view_from_relative;
        let [side_planes @ .., _far] =
            ViewFrustum::from_clip_from_world(&clip_from_relative).half_spaces;
        Self {
            clip_from_relative,
            frustum: side_planes.map(|plane| plane.normal_d()),
            camera_section: origin.section,
            camera_offset: origin.offset,
            viewport,
            focal_y: clip_from_view.y_axis.y,
        }
    }
}
