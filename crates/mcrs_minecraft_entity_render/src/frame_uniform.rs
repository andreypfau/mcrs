use bytemuck::{Pod, Zeroable};
use mcrs_minecraft_core::CardinalLight;
use mcrs_minecraft_mesh::face_shade;
use mcrs_minecraft_render::{FrameView, SIDE_PLANES};

/// The uniform of the entity passes in one frame. `shaders/common.wgsl` declares the same
/// layout as `Frame`.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Pod, Zeroable)]
pub struct FrameUniform {
    pub clip_from_relative: [f32; 16],
    pub frustum: [[f32; 4]; SIDE_PLANES],
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
    /// The [`face_shade::shader_index`] of the cardinal light of the dimension.
    pub shade_table: u32,
    pub scatter_count: u32,
    pub bone_capacity: u32,
    pub visible_capacity: u32,
    pub face_capacity: u32,
}

/// The height threshold of a full rig before any feedback, in pixels.
pub const FULL_RIG_HEIGHT: f32 = 12.0;

const _: () = assert!(size_of::<FrameUniform>() == 224);

impl FrameUniform {
    /// The camera fields of `view`, with the tick, the partial tick and the cardinal light of the
    /// dimension.
    #[must_use]
    pub fn new(view: &FrameView, tick: u32, partial_tick: f32, light: CardinalLight) -> Self {
        Self {
            clip_from_relative: view.clip_from_relative.to_cols_array(),
            frustum: view.frustum.map(|plane| plane.to_array()),
            camera_section: view.camera_section.to_array(),
            tick,
            camera_offset: view.camera_offset.to_array(),
            partial_tick,
            viewport: view.viewport.to_array(),
            projection_scale: view.focal_y * view.viewport.y * 0.5,
            height_threshold: FULL_RIG_HEIGHT,
            shade_table: face_shade::shader_index(light),
            ..Self::default()
        }
    }
}
