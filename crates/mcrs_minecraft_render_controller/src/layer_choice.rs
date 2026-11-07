/// The texture that a layer selects.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TextureChoice {
    /// A short name of the map `textures` of the client entity, by its index in
    /// [`crate::ClientEntityResources::textures`].
    Named(usize),
    /// A texture identifier that a string expression gave, as its `string_hash`.
    Identifier(u64),
}

/// What one layer of a client entity draws in a frame.
#[derive(Clone, Debug, PartialEq)]
pub struct LayerChoice {
    /// The index of the render controller entry of the client entity.
    pub entry: usize,
    /// The geometry, by its index in [`crate::ClientEntityResources::geometries`].
    pub geometry: usize,
    pub texture: TextureChoice,
    /// The `part_visibility` patterns in order, each with its value in this frame.
    pub part_visibility: Vec<(String, bool)>,
}
