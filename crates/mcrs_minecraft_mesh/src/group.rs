#[derive(Copy, Clone, Default, Debug, PartialEq, Eq, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
pub struct Group {
    pub quad_base: u32,
    pub quad_count: u32,
    pub section: u32,
    pub face: u32,
    pub bounds: u32,
}
