use bevy_ecs::entity::Entity;
use bevy_ecs::world::World;
use mcrs_minecraft_engine::persisted_registry::CaptureCause;
use mcrs_minecraft_engine::persisted_registry::PersistedError;
use mcrs_minecraft_nbt::tag::NbtTag;

use crate::capture_holders::capture_holders;
use crate::player_file_codec::NO_TABLE;
use crate::player_file_codec::decode_error;
use crate::player_file_codec::encode_error;
use crate::player_file_codec::item_table;
use crate::player_stacks::CURSOR_KEY;
use crate::player_stacks::PlayerStacks;
use crate::restore_holders::restore_carried;

/// The cursor and the craft grid, which only a cell crossing carries (INV-SLT-002). A leave and a
/// dimension change empty them first, and the player file never holds them.
pub(crate) fn capture_carried(
    world: &mut World,
    player: Entity,
    cause: CaptureCause,
) -> Result<Option<NbtTag>, PersistedError> {
    if cause != CaptureCause::CellCrossing {
        return Ok(None);
    }
    let table = item_table(world).ok_or_else(|| encode_error(CURSOR_KEY, NO_TABLE))?;
    let Some(stacks) = world
        .run_system_cached_with(capture_holders, (player, cause))
        .map_err(|error| encode_error(CURSOR_KEY, error))?
    else {
        return Ok(None);
    };
    let carried = stacks.write_carried(&table);
    Ok((!carried.is_empty()).then_some(NbtTag::Compound(carried)))
}

pub(crate) fn apply_carried(
    world: &mut World,
    player: Entity,
    tag: &NbtTag,
) -> Result<(), PersistedError> {
    let table = item_table(world).ok_or_else(|| decode_error(CURSOR_KEY, NO_TABLE))?;
    let NbtTag::Compound(carried) = tag else {
        return Err(decode_error(CURSOR_KEY, "expected a compound"));
    };
    let mut stacks = PlayerStacks::empty();
    stacks.read_carried(carried, &table);
    world
        .run_system_cached_with(restore_carried, (player, stacks))
        .map_err(|error| decode_error(CURSOR_KEY, error))
}
