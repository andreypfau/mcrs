use bytemuck::{Pod, Zeroable};
use mcrs_minecraft_core::CardinalLight;
use mcrs_minecraft_mesh::face_shade;
use mcrs_minecraft_render::{FrameView, SIDE_PLANES};

use crate::entity_views::EntityViews;

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Pod, Zeroable)]
pub struct FrameUniform {
    pub clip_from_relative: [f32; 16],
    pub frustum: [[f32; 4]; SIDE_PLANES],
    pub camera_section: [i32; 3],
    pub tick: u32,
    pub camera_offset: [f32; 3],
    pub partial_tick: f32,
    pub viewport: [f32; 2],
    pub projection_scale: f32,
    pub height_threshold: f32,
    pub opaque_zero: u32,
    pub slot_count: u32,
    pub hiz_levels: u32,
    pub shade_table: u32,
    pub scatter_count: u32,
    pub bone_capacity: u32,
    pub visible_capacity: u32,
    pub face_capacity: u32,
    pub view: u32,
    pub spare: [u32; 3],
}

pub const FULL_RIG_HEIGHT: f32 = 12.0;

const _: () = assert!(size_of::<FrameUniform>() == 240);

impl FrameUniform {
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
            view: EntityViews::WORLD.frame_bit(),
            ..Self::default()
        }
    }
}
