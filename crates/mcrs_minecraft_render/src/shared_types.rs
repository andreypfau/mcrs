use mcrs_minecraft_mesh::DrawKind;
use wgpu::util::DrawIndirectArgs;

pub type DrawArgs = DrawIndirectArgs;

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
#[repr(u32)]
pub enum DrawArgRow {
    FirstPassDraws = 0,
    SecondPassDraws = 1,
    HiddenQuads = 2,
    FirstPassSurvivors = 3,
    SecondPassSurvivors = 4,
    Candidates = 5,
    FirstPassQuadDispatch = 6,
    SecondPassQuadDispatch = 7,
    CandidateDispatch = 8,
}

impl DrawArgRow {
    pub const COUNT: usize = 9;
    pub const TOTAL_SLOTS: usize = Self::COUNT * DrawKind::COUNT;
    pub(crate) const DRAW_ARGS_BYTES: u64 = size_of::<DrawArgs>() as u64;

    #[must_use]
    pub const fn index(self) -> usize {
        self as usize
    }

    #[must_use]
    pub const fn first_slot(self) -> usize {
        self.index() * DrawKind::COUNT
    }

    #[must_use]
    pub const fn first_offset_bytes(self) -> u64 {
        self.first_slot() as u64 * Self::DRAW_ARGS_BYTES
    }

    #[must_use]
    pub const fn slot(self, draw: DrawKind) -> usize {
        self.first_slot() + draw.index()
    }

    #[must_use]
    pub const fn offset_bytes(self, draw: DrawKind) -> u64 {
        self.first_offset_bytes() + draw.index() as u64 * Self::DRAW_ARGS_BYTES
    }

    #[must_use]
    pub(crate) const fn byte_len(rows: usize) -> u64 {
        (rows * DrawKind::COUNT) as u64 * Self::DRAW_ARGS_BYTES
    }
}

pub const PARAMS_MODEL: u32 = 1;
pub const PARAMS_QUAD_CULL: u32 = 2;

#[derive(Copy, Clone, Default, Debug, PartialEq, Eq, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
pub struct Params {
    pub group_base: u32,
    pub group_count: u32,
    pub visible_base: u32,
    pub args_index: u32,
    pub counter: u32,
    pub flags: u32,
    pub(crate) _pad_24: [u32; 2],
}

pub(crate) const PARAMS_SIZE: u64 = size_of::<Params>() as u64;
const _: () = assert!(PARAMS_SIZE > 0);

#[derive(Copy, Clone, Default, Debug, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
pub struct CameraUniform {
    pub clip_from_relative: [f32; 16],
    pub frustum: [[f32; 4]; 5],
    pub section: [i32; 3],
    pub(crate) _pad_156: u32,
    pub offset: [f32; 3],
    pub(crate) _pad_172: u32,
    pub tint_origin: [f32; 2],
    pub tint_scale: [f32; 2],
    pub hiz_levels: u32,
    pub quad_cull: u32,
    pub viewport: [f32; 2],
    pub shade_table: u32,
    pub render_distance: u32,
    pub raster_viewport: [f32; 2],
    pub fog_color: [f32; 3],
    pub(crate) _pad_236: u32,
    pub fog_environmental: [f32; 2],
    pub fog_render_distance: [f32; 2],
}

pub(crate) const CAMERA_SIZE: u64 = size_of::<CameraUniform>() as u64;
const _: () = assert!(CAMERA_SIZE > 0);

#[derive(Copy, Clone, Default, Debug, PartialEq, Eq, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
pub struct SectionDesc {
    pub section: [i32; 3],
    pub scale: u32,
    pub face_base: u32,
}

pub(crate) const SECTION_BYTES: u64 = size_of::<SectionDesc>() as u64;

#[derive(Copy, Clone, Default, Debug, PartialEq, Eq, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
pub struct SpriteEntry {
    pub array_layer: u32,
    pub animation: u32,
}

#[derive(Copy, Clone, Default, Debug, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
pub(crate) struct AnimationFrame {
    pub layer: u32,
    pub next: u32,
    pub blend: f32,
    pub(crate) _pad_12: u32,
}

#[derive(Copy, Clone, Default, Debug, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
pub(crate) struct Reconstruction {
    pub relative_from_clip: [f32; 16],
    pub viewport: [f32; 2],
    pub(crate) _pad_72: [u32; 2],
}

pub(crate) const RECONSTRUCTION_SIZE: u64 = size_of::<Reconstruction>() as u64;
const _: () = assert!(RECONSTRUCTION_SIZE > 0);

#[derive(Copy, Clone, Default, Debug, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
pub struct SkyUniform {
    pub disc: [f32; 4],
    pub sunrise: [f32; 4],
    pub angles: [f32; 4],
    pub moon: [f32; 4],
    pub fog: [f32; 4],
    pub cloud_color: [f32; 4],
    pub cloud: [f32; 4],
    pub sky_light: [f32; 4],
    pub block_light: [f32; 4],
    pub ambient: [f32; 4],
    pub occluder: [f32; 4],
}

pub(crate) const SKY_SIZE: u64 = size_of::<SkyUniform>() as u64;
const _: () = assert!(SKY_SIZE > 0);

#[derive(Copy, Clone, Default, Debug, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
pub(crate) struct TextFrame {
    pub clip_from_relative: [f32; 16],
    pub section: [i32; 3],
    pub(crate) _pad_76: u32,
}

pub(crate) const TEXT_FRAME_SIZE: u64 = size_of::<TextFrame>() as u64;
const _: () = assert!(TEXT_FRAME_SIZE > 0);
