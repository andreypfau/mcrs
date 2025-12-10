use bevy_ecs::bundle::Bundle;
use bevy_ecs::entity::Entity;
use bevy_ecs::prelude::{Component, Query};
use bevy_ecs::query::Added;
use bevy_ecs::system::Res;
use mcrs_network::ServerSideConnection;
use mcrs_protocol::math::DVec3;
use mcrs_protocol::{ChunkPos, Position};
use crate::world::chunk::ChunkIndex;
use crate::world::chunk_observer::PlayerChunkObserver;
use crate::world::entity::{DeltaMovement, NetworkEntityId};

#[derive(Bundle)]
pub struct PrimedTntBundle {
    pub entity_marker: PrimedTntEntity,
    pub entity_id: NetworkEntityId,
    pub fuse: Fuse,
    pub delta_time: DeltaMovement
}

#[derive(Component)]
pub struct PrimedTntEntity;

impl Default for PrimedTntBundle {
    fn default() -> Self {
        let rot = rand::random::<f64>() * std::f64::consts::PI * 2.0;
        Self {
            entity_marker: PrimedTntEntity,
            entity_id: NetworkEntityId::default(),
            fuse: Fuse::default(),
            delta_time: DeltaMovement(
                DVec3::new(
                    -(rot.sin() * 0.02),
                    0.2,
                    -(rot.cos() * 0.02),
                )
            )
        }
    }
}

#[derive(Debug, Clone, Copy, Component)]
pub struct Fuse(pub u16);

impl Default for Fuse {
    fn default() -> Self {
        Self(80)
    }
}


fn send_spawn_primed_tnt(
    query: Query<(Entity, &Position), Added<PrimedTntEntity>>,
    players: Query<(&mut ServerSideConnection, &PlayerChunkObserver)>,
    chunk_index: Res<ChunkIndex>
) {
    for (entity, pos) in query.iter() {
        println!("Spawned primed TNT entity {:?} at position {:?}", entity, pos);
        let chunk_pos = ChunkPos::from(**pos);

    }
}