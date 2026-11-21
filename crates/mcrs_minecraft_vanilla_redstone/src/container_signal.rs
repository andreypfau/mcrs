use bevy_ecs::query::Changed;
use bevy_ecs::system::{Query, Res, ResMut};
use mcrs_minecraft_block::block_keys::Block;
use mcrs_minecraft_block::definition::Blocks;
use mcrs_minecraft_block_entity::{ContainerData, GeneratedBlockEntity};
use mcrs_minecraft_engine::block_simulation::{BlockMemory, BlockSimulation};
use mcrs_minecraft_engine::world_kinds::WorldKinds;
use mcrs_minecraft_engine_world::world::storage::block_entity::{BlockEntity, BlockEntityPos};
use mcrs_minecraft_item::ItemStackWithSlot;
use mcrs_minecraft_item::definition::Items;
use mcrs_minecraft_item_stack::{Kind, MaxStackSize};
use mcrs_minecraft_item_table::ItemTable;
use mcrs_minecraft_redstone::{AnalogSource, HasRedstoneMemory, MAX_SIGNAL};
use mcrs_minecraft_registries::Named;
use mcrs_minecraft_simulation::{BlockBehaviour, UpdateRequest};

const DEFAULT_STACK: i32 = 64;
const SLOT_LIMIT: i32 = 99;

fn max_stack(name: &str, items: &Items, table: &ItemTable) -> i32 {
    items
        .id_of(name)
        .and_then(|id| table.prototype(Kind(id.number())))
        .and_then(|prototype| prototype.get::<MaxStackSize>())
        .map_or(DEFAULT_STACK, |max| max.0.clamp(1, SLOT_LIMIT))
}

#[must_use]
pub fn container_signal(
    stacks: &[ItemStackWithSlot<Named>],
    slots: u8,
    items: &Items,
    table: &ItemTable,
) -> u8 {
    if slots == 0 {
        return 0;
    }
    let mut fill = 0.0_f32;
    let mut any = false;
    for entry in stacks {
        let count = entry.stack.count.0;
        if count <= 0 {
            continue;
        }
        any = true;
        let max = max_stack(entry.stack.item.as_str(), items, table);
        fill += count as f32 / max as f32;
    }
    if !any {
        return 0;
    }
    let fill = (fill / f32::from(slots)).min(1.0);
    let scaled = (fill * f32::from(MAX_SIGNAL - 1)).floor();
    let signal = u8::try_from(scaled as i32).unwrap_or(MAX_SIGNAL - 1);
    signal.saturating_add(1).min(MAX_SIGNAL)
}

fn container_of(entity: &GeneratedBlockEntity) -> Option<(Block, &ContainerData)> {
    match entity {
        GeneratedBlockEntity::Chest(container) => Some((Block::Chest, container)),
        GeneratedBlockEntity::TrappedChest(container) => Some((Block::TrappedChest, container)),
        GeneratedBlockEntity::Barrel(container) => Some((Block::Barrel, container)),
        GeneratedBlockEntity::Dispenser(container) => Some((Block::Dispenser, container)),
        GeneratedBlockEntity::Hopper(hopper) => Some((Block::Hopper, &hopper.container)),
        _ => None,
    }
}

pub(crate) fn publish_container_signals<B>(
    changed: Query<(&BlockEntityPos, &BlockEntity), Changed<BlockEntity>>,
    blocks: Res<Blocks>,
    items: Res<Items>,
    table: Res<ItemTable>,
    mut memory: ResMut<BlockMemory<B>>,
    mut simulation: ResMut<BlockSimulation>,
) where
    B: BlockBehaviour<WorldKinds> + Send + Sync + 'static,
    B::Memory: HasRedstoneMemory,
{
    for (pos, entity) in &changed {
        let Some((block, container)) = container_of(&entity.0) else {
            continue;
        };
        let slots = blocks
            .entry(block.id())
            .and_then(|entry| entry.container_slots)
            .unwrap_or(0);
        let signal = container_signal(&container.items, slots, &items, &table);
        let analog = &mut memory.0.redstone().analog;
        if analog.get(&pos.0).copied() == Some(AnalogSource::Container(signal)) {
            continue;
        }
        analog.insert(pos.0, AnalogSource::Container(signal));
        simulation.request_update(UpdateRequest::Comparators {
            source: pos.0,
            block: block.id(),
        });
    }
}
