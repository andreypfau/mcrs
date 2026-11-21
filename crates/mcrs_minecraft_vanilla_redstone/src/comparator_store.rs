use bevy_ecs::entity::Entity;
use bevy_ecs::query::{Added, With};
use bevy_ecs::system::{Commands, Query, ResMut};
use mcrs_minecraft_block_entity::GeneratedBlockEntity;
use mcrs_minecraft_engine::block_simulation::BlockMemory;
use mcrs_minecraft_engine::world_kinds::WorldKinds;
use mcrs_minecraft_engine_world::world::dimension::Dimension;
use mcrs_minecraft_engine_world::world::storage::block_entity::{
    BlockEntity, BlockEntityPos, SectionBlockEntities,
};
use mcrs_minecraft_engine_world::world::storage::section::SectionIndex;
use mcrs_minecraft_redstone::{HasRedstoneMemory, MAX_SIGNAL};
use mcrs_minecraft_simulation::BlockBehaviour;

use crate::block_entities::{entity_at, spawn_linked};

pub(crate) fn restore_comparator_outputs<B>(
    loaded: Query<(&BlockEntityPos, &BlockEntity), Added<BlockEntity>>,
    mut memory: ResMut<BlockMemory<B>>,
) where
    B: BlockBehaviour<WorldKinds> + Send + Sync + 'static,
    B::Memory: HasRedstoneMemory,
{
    for (pos, entity) in &loaded {
        let GeneratedBlockEntity::Comparator { output_signal, .. } = &entity.0 else {
            continue;
        };
        let signal = u8::try_from((*output_signal).clamp(0, i32::from(MAX_SIGNAL))).unwrap_or(0);
        memory
            .0
            .redstone()
            .comparators
            .entry(pos.0)
            .or_insert(signal);
    }
}

pub(crate) fn persist_comparator_outputs<B>(
    mut memory: ResMut<BlockMemory<B>>,
    dims: Query<(Entity, &SectionIndex), With<Dimension>>,
    sections: Query<&SectionBlockEntities>,
    positions: Query<&BlockEntityPos>,
    mut entities: Query<&mut BlockEntity>,
    mut commands: Commands,
) where
    B: BlockBehaviour<WorldKinds> + Send + Sync + 'static,
    B::Memory: HasRedstoneMemory,
{
    let redstone = memory.0.redstone();
    if redstone.comparator_dirty.is_empty() {
        return;
    }
    let Ok((dimension, index)) = dims.single() else {
        redstone.comparator_dirty.clear();
        return;
    };
    let mut spawned = Vec::new();
    for pos in redstone.comparator_dirty.drain(..) {
        let Some(signal) = redstone.comparators.get(&pos).copied() else {
            continue;
        };
        let found = entity_at(index, &sections, &positions, pos)
            .and_then(|entity| entities.get_mut(entity).ok());
        match found {
            Some(mut entity) => {
                if let GeneratedBlockEntity::Comparator { output_signal, .. } = &mut entity.0 {
                    *output_signal = i32::from(signal);
                }
            }
            None => spawned.push(GeneratedBlockEntity::Comparator {
                x: pos.x,
                y: pos.y,
                z: pos.z,
                output_signal: i32::from(signal),
                components: None,
            }),
        }
    }
    if !spawned.is_empty() {
        spawn_linked(&mut commands, dimension, index, spawned);
    }
}
