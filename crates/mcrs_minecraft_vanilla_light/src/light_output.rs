use mcrs_minecraft_light_engine::epoch::PublishedLight;
use mcrs_voxel::ColumnPos;

/// What a light stage hands back to its dimension, in the order it happened.
pub(crate) enum LightOutput {
    /// Sections whose light changed, each with only the layers that changed.
    Relit(Vec<PublishedLight>),
    /// Awaited columns whose light, and the light of the eight around them, has settled.
    Settled(Vec<ColumnPos>),
}
