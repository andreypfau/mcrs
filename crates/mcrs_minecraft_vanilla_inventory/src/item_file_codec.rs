use bevy_app::App;
use bevy_ecs::entity::Entity;
use bevy_ecs::world::World;
use mcrs_minecraft_engine::persisted_registry::CaptureCause;
use mcrs_minecraft_engine::persisted_registry::PersistScope;
use mcrs_minecraft_engine::persisted_registry::PersistedAppExt;
use mcrs_minecraft_engine::persisted_registry::PersistedError;
use mcrs_minecraft_engine_registries::RegistryNames;
use mcrs_minecraft_item_stack::SlotStack;
use mcrs_minecraft_item_stack::read_saved_with_slot;
use mcrs_minecraft_item_stack::write_saved_with_slot;
use mcrs_minecraft_nbt::tag::NbtTag;

use crate::EnderItems;
use crate::player_file_codec::decode_error;
use crate::player_file_codec::encode_error;
use crate::player_file_codec::registry_names;

const ENDER_SLOTS: u8 = 27;

const ENDER_ITEMS: &str = "EnderItems";

pub(crate) fn register_ender_items(app: &mut App) {
    app.register_persisted_codec(
        &[ENDER_ITEMS],
        PersistScope::Persisted,
        capture_ender_items,
        apply_ender_items,
    );
}

fn slot_stacks(
    tag: &NbtTag,
    names: &RegistryNames<'_>,
    key: &'static str,
) -> Result<Vec<SlotStack>, PersistedError> {
    let NbtTag::List(entries) = tag else {
        return Err(decode_error(key, "expected a list of slot stacks"));
    };
    entries
        .iter()
        .map(|entry| {
            let NbtTag::Compound(compound) = entry else {
                return Err(decode_error(key, "expected a slot stack compound"));
            };
            read_saved_with_slot(compound, names).map_err(|error| decode_error(key, error))
        })
        .collect()
}

fn capture_ender_items(
    world: &World,
    holder: Entity,
    _: CaptureCause,
) -> Result<Option<NbtTag>, PersistedError> {
    let Some(items) = world.get::<EnderItems>(holder) else {
        return Ok(None);
    };
    let names = registry_names(world).map_err(|missing| encode_error(ENDER_ITEMS, missing))?;
    let list = items
        .0
        .iter()
        .map(|slotted| {
            write_saved_with_slot(slotted, &names)
                .map(NbtTag::Compound)
                .map_err(|error| encode_error(ENDER_ITEMS, error))
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Some(NbtTag::List(list)))
}

fn apply_ender_items(
    world: &mut World,
    holder: Entity,
    tag: &NbtTag,
) -> Result<(), PersistedError> {
    let names = registry_names(world).map_err(|missing| decode_error(ENDER_ITEMS, missing))?;
    let mut items = slot_stacks(tag, &names, ENDER_ITEMS)?;
    items.retain(|slotted| slotted.slot < ENDER_SLOTS);
    world
        .get_entity_mut(holder)
        .map_err(|_| PersistedError::NoEntity(holder))?
        .insert(EnderItems(items));
    Ok(())
}
