// Generated from `GROUP_FIELDS` in pack.rs. `cargo nextest run -p mcrs_minecraft_mesh`
// checks it; `MCRS_BLESS=1 cargo nextest run -p mcrs_minecraft_mesh` rewrites it.

/// A batch of up to [`crate::GROUP_QUADS`] quads in one section and face, tested together by GPU culling.
#[derive(Copy, Clone, Default, Debug, PartialEq, Eq, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
pub struct Group {
    pub quad_base: u32,
    pub quad_count: u32,
    pub section: u32,
    pub face: u32,
    /// The box its quads lie in, packed by the `BOUNDS_*` fields.
    pub bounds: u32,
}
