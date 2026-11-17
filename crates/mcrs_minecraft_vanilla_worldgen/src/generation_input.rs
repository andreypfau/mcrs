use mcrs_minecraft_engine_world::entity::player::chunk_view::ChunkTrackingView;
use mcrs_minecraft_worldgen_generator::stages::FillContext;
use mcrs_voxel::ColumnPos;

pub(crate) enum GenerationInput {
    Context(Box<FillContext>),
    Want(ColumnPos),
    Views(Vec<ChunkTrackingView>),
}
