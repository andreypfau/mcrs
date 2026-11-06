use mcrs_minecraft_engine_world::entity::player::chunk_view::ChunkTrackingView;
use mcrs_minecraft_worldgen_generator::stages::FillContext;
use mcrs_voxel::ColumnPos;

/// What the world sends to its generation stage.
pub(crate) enum GenerationInput {
    /// The context every ladder stage runs against. Until it arrives the stage queues what is
    /// wanted and generates nothing.
    Context(Box<FillContext>),
    /// A column that a cell asked for, ordered by its distance to the nearest view center.
    Want(ColumnPos),
    /// What the views are this tick. The stage drops what left every view and orders what is
    /// left by the distance to the view centers.
    Views(Vec<ChunkTrackingView>),
}
