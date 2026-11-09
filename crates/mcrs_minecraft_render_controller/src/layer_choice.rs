/// The texture that a layer selects.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TextureChoice {
    /// A short name of the map `textures` of the client entity, by its index in
    /// [`mcrs_minecraft_pack_file::ClientEntityFile::textures`].
    Named(usize),
    /// A texture identifier that a string expression gave, as its `string_hash`.
    Identifier(u64),
}

/// The most `part_visibility` patterns of one layer.
pub const MAX_PART_PATTERNS: usize = 64;

/// What one layer of a client entity draws in a frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LayerChoice {
    /// The index of the layer in [`crate::RenderControllers::layers`].
    pub entry: usize,
    /// The geometry, by its index in [`mcrs_minecraft_pack_file::ClientEntityFile::geometries`].
    pub geometry: usize,
    pub texture: TextureChoice,
    /// Bit `k` is set when the pattern `k` of the `part_visibility` of the layer shows its
    /// bones in this frame.
    pub shown_parts: u64,
}

impl LayerChoice {
    /// Whether the `part_visibility` pattern `index` of the layer shows its bones.
    #[must_use]
    pub fn shows_part(&self, index: usize) -> bool {
        index < MAX_PART_PATTERNS && self.shown_parts & (1 << index) != 0
    }
}
