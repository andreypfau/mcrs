/// Groups and quads belonging to one [`crate::DrawKind`] inside a meshed section.
#[derive(Copy, Clone, Default, Debug, PartialEq, Eq)]
pub struct SectionDraw {
    pub group_count: u32,
    pub quad_count: u32,
}
