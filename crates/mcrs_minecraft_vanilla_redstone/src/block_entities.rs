use bevy_ecs::entity::Entity;
use bevy_ecs::system::{Commands, Query};
use mcrs_minecraft_block_entity::GeneratedBlockEntity;
use mcrs_minecraft_engine_world::world::dimension::InDimension;
use mcrs_minecraft_engine_world::world::storage::block_entity::{
    BlockEntity, BlockEntityPos, InSection, SectionBlockEntities,
};
use mcrs_minecraft_engine_world::world::storage::section::SectionIndex;
use mcrs_voxel::{BlockPos, SectionPos};

pub(crate) fn spawn_linked(
    commands: &mut Commands,
    dimension: Entity,
    index: &SectionIndex,
    entries: Vec<GeneratedBlockEntity>,
) {
    for entry in entries {
        let pos = entry.position();
        let at = BlockPos::new(pos.x, pos.y, pos.z);
        let mut spawned = commands.spawn((
            BlockEntityPos(at),
            InDimension(dimension),
            BlockEntity(entry),
        ));
        if let Some(section) = index.get(SectionPos::from(at)) {
            spawned.insert(InSection(section));
        }
    }
}

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
