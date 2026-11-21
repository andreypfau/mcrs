use bevy_ecs::entity::Entity;
use bevy_ecs::system::Query;
use mcrs_minecraft_engine_world::world::storage::block_entity::{
    BlockEntityPos, SectionBlockEntities,
};
use mcrs_minecraft_engine_world::world::storage::section::SectionIndex;
use mcrs_voxel::{BlockPos, SectionPos};

pub(crate) fn entity_at(
    index: &SectionIndex,
    sections: &Query<&SectionBlockEntities>,
    positions: &Query<&BlockEntityPos>,
    pos: BlockPos,
) -> Option<Entity> {
    let section = index.get(SectionPos::from(pos))?;
    let held = sections.get(section).ok()?;
    held.iter()
        .copied()
        .find(|entity| positions.get(*entity).is_ok_and(|at| at.0 == pos))
}
