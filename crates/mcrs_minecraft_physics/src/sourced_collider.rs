use mcrs_minecraft_entity_key::EntityId;
use mcrs_voxel::BlockPos;
use mcrs_voxel::BlockStateId;

use crate::Collider;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ColliderSource {
    Block { pos: BlockPos, state: BlockStateId },
    Entity(EntityId),
    Border,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SourcedCollider {
    pub collider: Collider,
    pub source: ColliderSource,
}
