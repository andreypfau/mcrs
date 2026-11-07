use bytemuck::{Pod, Zeroable};
use glam::{IVec3, Mat4, Vec3};

/// The uniform of the entity passes in one frame. `shaders/common.wgsl` declares the same
/// layout as `Frame`.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Pod, Zeroable)]
pub struct FrameUniform {
    pub clip_from_relative: [f32; 16],
    pub frustum: [[f32; 4]; 5],
    pub camera_section: [i32; 3],
    /// The client tick of the frame.
    pub tick: u32,
    /// The eye relative to the origin of the camera section.
    pub camera_offset: [f32; 3],
    pub partial_tick: f32,
    pub viewport: [f32; 2],
    /// `H / (2 · tan(fov_y / 2))`: the pixels of one block at the distance of one block.
    pub projection_scale: f32,
    /// The projected height in pixels from which a mob draws its full rig.
    pub height_threshold: f32,
    /// A zero that the shader compiler cannot see.
    pub opaque_zero: u32,
    pub slot_count: u32,
    pub hiz_levels: u32,
    /// [`FRAME_NETHER_LIGHT`].
    pub flags: u32,
    pub scatter_count: u32,
    pub bone_capacity: u32,
    pub visible_capacity: u32,
    pub face_capacity: u32,
}

pub const FRAME_NETHER_LIGHT: u32 = 1;
/// The height threshold of a full rig before any feedback, in pixels.
pub const FULL_RIG_HEIGHT: f32 = 12.0;

const _: () = assert!(size_of::<FrameUniform>() == 224);

/// The view of a frame: the projection relative to the origin of the camera section.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FrameView {
    pub clip_from_relative: Mat4,
    pub camera_section: IVec3,
    pub camera_offset: Vec3,
    pub viewport: [f32; 2],
    /// The element `[1][1]` of the projection: `1 / tan(fov_y / 2)`.
    pub focal_y: f32,
}

impl FrameUniform {
    /// The camera fields of `view`, with the tick, the partial tick and the light table.
    #[must_use]
    pub fn new(view: &FrameView, tick: u32, partial_tick: f32, nether_light: bool) -> Self {
        let planes = frustum_planes(&view.clip_from_relative);
        Self {
            clip_from_relative: view.clip_from_relative.to_cols_array(),
            frustum: planes,
            camera_section: view.camera_section.to_array(),
            tick,
            camera_offset: view.camera_offset.to_array(),
            partial_tick,
            viewport: view.viewport,
            projection_scale: view.focal_y * view.viewport[1] * 0.5,
            height_threshold: FULL_RIG_HEIGHT,
            flags: if nether_light { FRAME_NETHER_LIGHT } else { 0 },
            ..Self::default()
        }
    }
}

/// The left, right, bottom, top and near planes of a reverse-Z infinite projection, each as
/// `(normal, d)` with the inside where `normal · p + d > 0`.
fn frustum_planes(clip: &Mat4) -> [[f32; 4]; 5] {
    let rows = [clip.row(0), clip.row(1), clip.row(2), clip.row(3)];
    let [x, y, z, w] = rows;
    let planes = [w + x, w - x, w + y, w - y, w - z];
    planes.map(|plane| {
        let length = plane.truncate().length().max(f32::MIN_POSITIVE);
        (plane / length).to_array()
    })
}
