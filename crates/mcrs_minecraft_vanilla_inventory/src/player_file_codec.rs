use std::fmt::Display;

use bevy_ecs::entity::Entity;
use bevy_ecs::world::World;
use mcrs_minecraft_engine::persisted_registry::CaptureCause;
use mcrs_minecraft_engine::persisted_registry::PersistedError;
use mcrs_minecraft_engine_registries::RegistryNames;
use mcrs_minecraft_inventory_file::FileError;
use mcrs_minecraft_inventory_file::PlayerInventoryFile;
use mcrs_minecraft_inventory_player::MAIN;
use mcrs_minecraft_item_table::ItemTable;
use mcrs_minecraft_nbt::compound::NbtCompound;
use mcrs_minecraft_nbt::tag::NbtTag;
use mcrs_minecraft_player_holders::restore_holders;
use mcrs_minecraft_registries::CurrentRegistries;

use crate::capture_holders::capture_holders;
use crate::kept_equipment::KeptEquipment;
use crate::player_file::INVENTORY_KEY;

pub(crate) fn capture_player_file(
    world: &World,
    player: Entity,
    _: CaptureCause,
) -> Result<Option<NbtTag>, PersistedError> {
    let names = registry_names(world).map_err(|missing| encode_error(INVENTORY_KEY, missing))?;
    let Some(items) = capture_holders(world, player) else {
        return Ok(None);
    };
    let file = PlayerInventoryFile {
        items,
        kept_equipment: world
            .get::<KeptEquipment>(player)
            .map(|kept| kept.0.clone())
            .unwrap_or_default(),
    };
    let mut compound = NbtCompound::new();
    mcrs_minecraft_inventory_file::write(&file, &names, &mut compound)
        .map_err(|error| encode_error(file_key(error), error))?;
    Ok(Some(NbtTag::Compound(compound)))
}

pub(crate) fn apply_player_file(
    world: &mut World,
    player: Entity,
    tag: &NbtTag,
) -> Result<(), PersistedError> {
    let names = registry_names(world).map_err(|missing| decode_error(INVENTORY_KEY, missing))?;
    let NbtTag::Compound(compound) = tag else {
        return Err(decode_error(INVENTORY_KEY, FileError::NotACompound));
    };
    let file = mcrs_minecraft_inventory_file::read(compound, &names)
        .map_err(|error| decode_error(file_key(error), error))?;
    let saved = compound.get_list(INVENTORY_KEY).map_or(0, <[NbtTag]>::len);
    let loaded = file
        .items
        .slots
        .iter()
        .take(usize::from(MAIN.end))
        .filter(|slot| slot.is_some())
        .count();
    if saved != loaded {
        tracing::error!(
            %player,
            saved,
            loaded,
            "stacks of `Inventory` with no slot in the main inventory are lost"
        );
    }
    world
        .get_entity_mut(player)
        .map_err(|_| PersistedError::NoEntity(player))?
        .insert(KeptEquipment(file.kept_equipment));
    world
        .run_system_cached_with(restore_holders, (player, file.items))
        .map_err(|error| decode_error(INVENTORY_KEY, error))
}

const NO_TABLE: &str = "the world has no item table";

const NO_REGISTRIES: &str = "the world has no registries";

pub(crate) fn registry_names(world: &World) -> Result<RegistryNames<'_>, &'static str> {
    let table = world.get_resource::<ItemTable>().ok_or(NO_TABLE)?;
    let registries = world
        .get_resource::<CurrentRegistries>()
        .ok_or(NO_REGISTRIES)?;
    Ok(RegistryNames::new(registries, table))
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

fn file_key(error: FileError) -> &'static str {
    match error {
        FileError::Stack { key, .. } | FileError::BadShape(key) => key,
        FileError::NotACompound => INVENTORY_KEY,
    }
}
