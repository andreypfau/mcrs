use bevy_ecs::entity::Entity;
use bevy_ecs::resource::Resource;
use mcrs_engine_muxer::TileDeltas;
use mcrs_minecraft_replication::ReplicationData;

/// What the host keeps of each dimension's tile deltas, for a test that compares them. Absent
/// in a server, which keeps none.
#[derive(Resource, Default, Debug)]
pub struct TileDeltaLog(pub Vec<LoggedDeltas>);

#[derive(Debug)]
pub struct LoggedDeltas {
    pub dimension: Entity,
    pub deltas: TileDeltas<ReplicationData>,
}
