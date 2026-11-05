use mcrs_minecraft_engine_world::entity::player::chunk_view::ChunkTrackingView;
use mcrs_minecraft_worldgen_generator::stages::FillContext;
use mcrs_voxel::ColumnPos;

/// What the world sends to its generation stage.
pub(crate) enum GenerationInput {
    /// The context every ladder stage runs against. Until it arrives the stage queues what is
    /// wanted and generates nothing.
    Context(Box<FillContext>),
    Want(WantedColumn),
    /// What the views and the players are this tick. The stage drops what left every view and
    /// orders what is left by the players' distance.
    Views(ViewUpdate),
}

/// A column that a cell asked for.
pub(crate) struct WantedColumn {
    pub(crate) col: ColumnPos,
    /// Squared XZ distance to the nearest player when the column was asked for.
    pub(crate) distance_sq: i32,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct ViewUpdate {
    pub(crate) views: Vec<ChunkTrackingView>,
    pub(crate) players: Vec<ColumnPos>,
}
