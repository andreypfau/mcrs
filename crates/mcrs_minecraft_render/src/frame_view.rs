use bevy::prelude::*;
use bevy::shape::ViewFrustum;

use crate::frame::CameraOrigin;

pub const SIDE_PLANES: usize = 5;

#[derive(Resource, Clone, Copy, Debug, PartialEq)]
pub struct FrameView {
    pub clip_from_relative: Mat4,
    pub frustum: [Vec4; SIDE_PLANES],
    pub camera_section: IVec3,
    pub camera_offset: Vec3,
    pub viewport: Vec2,
    pub focal_y: f32,
}

impl FrameView {
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
