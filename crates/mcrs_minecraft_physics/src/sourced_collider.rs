use mcrs_engine_muxer::EntityId;
use mcrs_voxel::BlockPos;
use mcrs_voxel::BlockStateId;

use crate::Collider;

/// The source of a collider of a query.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ColliderSource {
    Block { pos: BlockPos, state: BlockStateId },
    Entity(EntityId),
    Border,
}

/// A collider with its source.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SourcedCollider {
    pub collider: Collider,
    pub source: ColliderSource,
}
