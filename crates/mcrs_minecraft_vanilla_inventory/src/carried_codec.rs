use bevy_ecs::entity::Entity;
use bevy_ecs::world::World;
use mcrs_minecraft_engine::persisted_registry::CaptureCause;
use mcrs_minecraft_engine::persisted_registry::PersistedError;
use mcrs_minecraft_engine_registries::RegistryNames;
use mcrs_minecraft_inventory_file::PlayerItems;
use mcrs_minecraft_nbt::compound::NbtCompound;
use mcrs_minecraft_nbt::tag::NbtTag;
use mcrs_minecraft_player_holders::restore_carried;

use crate::capture_holders::capture_holders;
use crate::player_file_codec::decode_error;
use crate::player_file_codec::encode_error;
use crate::player_file_codec::registry_names;
use crate::saved_stack;

const CURSOR_KEY: &str = "mcrs:cursor";
const CRAFT_GRID_KEY: &str = "mcrs:craft_grid";
pub(crate) const CARRIED_KEYS: &[&str] = &[CURSOR_KEY, CRAFT_GRID_KEY];

pub(crate) fn capture_carried(
    world: &World,
    player: Entity,
    cause: CaptureCause,
) -> Result<Option<NbtTag>, PersistedError> {
    if cause != CaptureCause::CellCrossing {
        return Ok(None);
    }
    let names = registry_names(world).map_err(|missing| encode_error(CURSOR_KEY, missing))?;
    let Some(items) = capture_holders(world, player) else {
        return Ok(None);
    };
    let carried = write_carried(&items, &names)?;
    Ok((!carried.is_empty()).then_some(NbtTag::Compound(carried)))
}

pub(crate) fn apply_carried(
    world: &mut World,
    player: Entity,
    tag: &NbtTag,
) -> Result<(), PersistedError> {
    let names = registry_names(world).map_err(|missing| decode_error(CURSOR_KEY, missing))?;
    let NbtTag::Compound(carried) = tag else {
        return Err(decode_error(CURSOR_KEY, "expected a compound"));
    };
    let mut items = PlayerItems::default();
    read_carried(&mut items, carried, &names)?;
    world
        .run_system_cached_with(restore_carried, (player, items))
        .map_err(|error| decode_error(CURSOR_KEY, error))
}

pub(crate) fn has_carried(items: &PlayerItems) -> bool {
    items.cursor.is_some() || items.craft_grid.iter().any(Option::is_some)
}

fn read_carried(
    items: &mut PlayerItems,
    carried: &NbtCompound,
    names: &RegistryNames<'_>,
) -> Result<(), PersistedError> {
    if let Some(saved) = carried.get_compound(CURSOR_KEY) {
        items.cursor = Some(saved_stack::from_saved_with_slot(CURSOR_KEY, saved, names)?.stack);
    }
    if let Some(NbtTag::List(craft_grid)) = carried.get(CRAFT_GRID_KEY) {
        for tag in craft_grid {
            let NbtTag::Compound(saved) = tag else {
                continue;
            };
            let slotted = saved_stack::from_saved_with_slot(CRAFT_GRID_KEY, saved, names)?;
            if let Some(slot) = items.craft_grid.get_mut(usize::from(slotted.slot)) {
                *slot = Some(slotted.stack);
            }
        }
    }
    Ok(())
}

fn write_carried(
    items: &PlayerItems,
    names: &RegistryNames<'_>,
) -> Result<NbtCompound, PersistedError> {
    let mut carried = NbtCompound::new();
    if let Some(cursor) = &items.cursor {
        let saved = saved_stack::to_saved_with_slot(CURSOR_KEY, 0, cursor, names)?;
        carried.put(CURSOR_KEY, NbtTag::Compound(saved));
    }
    let mut craft_grid = Vec::new();
    for (slot, stack) in (0..=u8::MAX).zip(&items.craft_grid) {
        if let Some(stack) = stack {
            let saved = saved_stack::to_saved_with_slot(CRAFT_GRID_KEY, slot, stack, names)?;
            craft_grid.push(NbtTag::Compound(saved));
        }
    }
    if !craft_grid.is_empty() {
        carried.put_list(CRAFT_GRID_KEY, craft_grid);
    }
    Ok(carried)
}
