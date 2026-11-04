use mcrs_minecraft_light_engine::epoch::{PublishedLight, SectionLight};
use mcrs_voxel::ColumnPos;

/// What a light stage hands back to its dimension, in the order it happened.
pub(crate) enum LightOutput {
    /// Sections whose light changed, each with only the layers that changed.
    Relit(Vec<PublishedLight>),
    /// Awaited columns whose light, and the light of the eight around them, has settled.
    Settled(Vec<ColumnPos>),
    /// Columns awaited with their light, with the light of each of their loaded sections once
    /// the light of the eight around them has settled too.
    Lit(Vec<LitColumn>),
}

/// The settled light of one column, both layers of each section, bottom first.
pub(crate) struct LitColumn {
    pub(crate) column: ColumnPos,
    pub(crate) sections: Vec<SectionLight>,
}
