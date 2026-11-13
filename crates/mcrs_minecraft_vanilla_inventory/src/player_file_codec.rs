use std::fmt::Display;

use bevy_ecs::entity::Entity;
use bevy_ecs::world::World;
use mcrs_minecraft_engine::persisted_registry::CaptureCause;
use mcrs_minecraft_engine::persisted_registry::PersistedError;
use mcrs_minecraft_item_table::ItemTable;
use mcrs_minecraft_nbt::tag::NbtTag;

use crate::capture_holders::capture_holders;
use crate::kept_equipment::KeptEquipment;
use crate::player_file::INVENTORY_KEY;
use crate::player_file::PlayerFile;
use crate::player_stacks::PlayerStacks;
use crate::player_stacks::is_slot_key;
use crate::restore_holders::restore_holders;

/// The player inventory, its selected slot and the kept keys of `equipment`, after the close that
/// `cause` asks for.
pub(crate) fn capture_player_file(
    world: &mut World,
    player: Entity,
    cause: CaptureCause,
) -> Result<Option<NbtTag>, PersistedError> {
    let table = item_table(world).ok_or_else(|| encode_error(INVENTORY_KEY, NO_TABLE))?;
    let Some(stacks) = world
        .run_system_cached_with(capture_holders, (player, cause))
        .map_err(|error| encode_error(INVENTORY_KEY, error))?
    else {
        return Ok(None);
    };
    let mut file = PlayerFile::default();
    stacks.write(&mut file, &table);
    if let Some(kept) = world.get::<KeptEquipment>(player) {
        for (key, saved) in &kept.0.child_tags {
            file.equipment.put(key, saved.clone());
        }
    }
    mcrs_minecraft_nbt::to_nbt_tag(&file)
        .map(Some)
        .map_err(|source| PersistedError::Encode {
            key: INVENTORY_KEY,
            source,
        })
}

pub(crate) fn apply_player_file(
    world: &mut World,
    player: Entity,
    tag: &NbtTag,
) -> Result<(), PersistedError> {
    let table = item_table(world).ok_or_else(|| decode_error(INVENTORY_KEY, NO_TABLE))?;
    let file: PlayerFile =
        mcrs_minecraft_nbt::from_tag(tag.clone()).map_err(|source| PersistedError::Decode {
            key: INVENTORY_KEY,
            source,
        })?;
    let stacks = PlayerStacks::read(&file, &table);
    let mut kept = file.equipment;
    kept.child_tags.retain(|(key, _)| !is_slot_key(key));
    world
        .get_entity_mut(player)
        .map_err(|_| PersistedError::NoEntity(player))?
        .insert(KeptEquipment(kept));
    world
        .run_system_cached_with(restore_holders, (player, stacks))
        .map_err(|error| decode_error(INVENTORY_KEY, error))
}

pub(crate) const NO_TABLE: &str = "the world has no item table";

pub(crate) fn item_table(world: &World) -> Option<ItemTable> {
    world.get_resource::<ItemTable>().copied()
}

pub(crate) fn encode_error(key: &'static str, cause: impl Display) -> PersistedError {
    PersistedError::Encode {
        key,
        source: mcrs_minecraft_nbt::Error::SerdeError(cause.to_string()),
    }
}

pub(crate) fn decode_error(key: &'static str, cause: impl Display) -> PersistedError {
    PersistedError::Decode {
        key,
        source: mcrs_minecraft_nbt::Error::SerdeError(cause.to_string()),
    }
}
