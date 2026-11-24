use bevy_app::App;
use bevy_ecs::entity::Entity;
use bevy_ecs::world::World;
use mcrs_minecraft_engine::persisted_registry::CaptureCause;
use mcrs_minecraft_engine::persisted_registry::PersistScope;
use mcrs_minecraft_engine::persisted_registry::PersistedAppExt;
use mcrs_minecraft_engine::persisted_registry::PersistedError;
use mcrs_minecraft_engine::persisted_registry::PersistedForm;
use mcrs_minecraft_engine_item::HolderCommandsExt;
use mcrs_minecraft_engine_item::Selected;
use mcrs_minecraft_engine_item::Slots;
use mcrs_minecraft_engine_item::Stacks;
use mcrs_minecraft_engine_registries::RegistryNames;
use mcrs_minecraft_item_stack::ItemStack;
use mcrs_minecraft_item_stack::SlotStack;
use mcrs_minecraft_item_stack::read_saved;
use mcrs_minecraft_item_stack::read_saved_with_slot;
use mcrs_minecraft_item_stack::write_saved;
use mcrs_minecraft_item_stack::write_saved_with_slot;
use mcrs_minecraft_item_table::ItemTable;
use mcrs_minecraft_nbt::Error;
use mcrs_minecraft_nbt::compound::NbtCompound;
use mcrs_minecraft_nbt::tag::NbtTag;
use mcrs_minecraft_registries::CurrentRegistries;
use serde::Deserialize;
use serde::Serialize;

use crate::EnderItems;
use crate::WornEquipment;

const INVENTORY_SLOTS: usize = 36;
const ENDER_SLOTS: u8 = 27;
const HOTBAR_SLOTS: u8 = 9;

const INVENTORY: &str = "Inventory";
const ENDER_ITEMS: &str = "EnderItems";
const EQUIPMENT: &str = "equipment";
const SELECTED_ITEM_SLOT: &str = "SelectedItemSlot";

#[derive(Serialize, Deserialize)]
struct SelectedItemSlot(i32);

impl PersistedForm<Selected> for SelectedItemSlot {
    fn from_value(selected: &Selected) -> Self {
        Self(i32::from(selected.0))
    }

    fn into_value(self) -> Result<Selected, String> {
        u8::try_from(self.0)
            .ok()
            .filter(|slot| *slot < HOTBAR_SLOTS)
            .map(Selected)
            .ok_or_else(|| format!("no hotbar slot {}", self.0))
    }
}

pub(crate) fn register_ender_items(app: &mut App) {
    app.register_persisted_codec(
        &[ENDER_ITEMS],
        PersistScope::Persisted,
        capture_ender_items,
        apply_ender_items,
    );
}

pub(crate) fn register_player_items(app: &mut App) {
    app.register_persisted_as::<Selected, SelectedItemSlot>(
        &[SELECTED_ITEM_SLOT],
        PersistScope::Persisted,
    )
    .register_persisted_codec(
        &[INVENTORY],
        PersistScope::Persisted,
        capture_inventory,
        apply_inventory,
    )
    .register_persisted_codec(
        &[EQUIPMENT],
        PersistScope::Persisted,
        capture_equipment,
        apply_equipment,
    );
}

fn encode_error(key: &'static str, cause: impl ToString) -> PersistedError {
    PersistedError::Encode {
        key,
        source: Error::SerdeError(cause.to_string()),
    }
}

fn decode_error(key: &'static str, cause: impl ToString) -> PersistedError {
    PersistedError::Decode {
        key,
        source: Error::SerdeError(cause.to_string()),
    }
}

fn registry_names<'world>(
    world: &'world World,
    key: &'static str,
) -> Result<RegistryNames<'world>, PersistedError> {
    let table = world
        .get_resource::<ItemTable>()
        .ok_or_else(|| encode_error(key, "the world has no item table"))?;
    let registries = world
        .get_resource::<CurrentRegistries>()
        .ok_or_else(|| encode_error(key, "the world has no registries"))?;
    Ok(RegistryNames::new(registries, table))
}

fn capture_inventory(
    world: &World,
    holder: Entity,
    _: CaptureCause,
) -> Result<Option<NbtTag>, PersistedError> {
    let Some(slots) = world.get::<Slots>(holder) else {
        return Ok(None);
    };
    let placed = (0..slots.len().min(INVENTORY_SLOTS))
        .filter_map(|index| Some((u8::try_from(index).ok()?, slots.get(index)?)));
    let names = registry_names(world, INVENTORY)?;
    let mut list = Vec::new();
    for (slot, stack) in placed {
        let Some(stack) = Stacks::value_in(world, stack) else {
            continue;
        };
        let saved = write_saved_with_slot(&SlotStack { slot, stack }, &names)
            .map_err(|error| encode_error(INVENTORY, error))?;
        list.push(NbtTag::Compound(saved));
    }
    Ok(Some(NbtTag::List(list)))
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

fn apply_inventory(world: &mut World, holder: Entity, tag: &NbtTag) -> Result<(), PersistedError> {
    let names = registry_names(world, INVENTORY)?;
    let mut values: Vec<Option<ItemStack>> = vec![None; INVENTORY_SLOTS];
    for SlotStack { slot, stack } in slot_stacks(tag, &names, INVENTORY)? {
        if let Some(value) = values.get_mut(usize::from(slot)) {
            *value = Some(stack);
        }
    }
    world.commands().entity(holder).load_inventory(values);
    world.flush();
    Ok(())
}

fn capture_ender_items(
    world: &World,
    holder: Entity,
    _: CaptureCause,
) -> Result<Option<NbtTag>, PersistedError> {
    let Some(items) = world.get::<EnderItems>(holder) else {
        return Ok(None);
    };
    let names = registry_names(world, ENDER_ITEMS)?;
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
    let names = registry_names(world, ENDER_ITEMS)?;
    let mut items = slot_stacks(tag, &names, ENDER_ITEMS)?;
    items.retain(|slotted| slotted.slot < ENDER_SLOTS);
    world
        .get_entity_mut(holder)
        .map_err(|_| PersistedError::NoEntity(holder))?
        .insert(EnderItems(items));
    Ok(())
}

fn capture_equipment(
    world: &World,
    holder: Entity,
    _: CaptureCause,
) -> Result<Option<NbtTag>, PersistedError> {
    let worn = world.get::<WornEquipment>(holder).cloned();
    let selected_holds_stack = match (world.get::<Slots>(holder), world.get::<Selected>(holder)) {
        (Some(slots), Some(selected)) => slots.get(usize::from(selected.0)).is_some(),
        _ => false,
    };
    let mut worn = match worn {
        Some(worn) => worn,
        None if selected_holds_stack => WornEquipment::default(),
        None => return Ok(None),
    };
    let names = registry_names(world, EQUIPMENT)?;
    let mut compound = NbtCompound::new();
    for (key, stack) in worn_slots(&mut worn) {
        if let Some(stack) = stack.as_ref() {
            let saved =
                write_saved(stack, &names).map_err(|error| encode_error(EQUIPMENT, error))?;
            compound.put(key, NbtTag::Compound(saved));
        }
    }
    if compound.is_empty() && !selected_holds_stack {
        return Ok(None);
    }
    Ok(Some(NbtTag::Compound(compound)))
}

fn apply_equipment(world: &mut World, holder: Entity, tag: &NbtTag) -> Result<(), PersistedError> {
    let NbtTag::Compound(compound) = tag else {
        return Err(decode_error(EQUIPMENT, "expected a compound"));
    };
    let names = registry_names(world, EQUIPMENT)?;
    let mut worn = WornEquipment::default();
    for (key, entry) in &compound.child_tags {
        let NbtTag::Compound(saved) = entry else {
            return Err(decode_error(EQUIPMENT, "expected an item stack compound"));
        };
        if key == "mainhand" {
            read_saved(saved, &names).map_err(|error| decode_error(EQUIPMENT, error))?;
            continue;
        }
        let Some((_, slot)) = worn_slots(&mut worn)
            .into_iter()
            .find(|(name, _)| name == key)
        else {
            return Err(decode_error(EQUIPMENT, format!("unknown slot `{key}`")));
        };
        *slot = Some(read_saved(saved, &names).map_err(|error| decode_error(EQUIPMENT, error))?);
    }
    world
        .get_entity_mut(holder)
        .map_err(|_| PersistedError::NoEntity(holder))?
        .insert(worn);
    Ok(())
}

fn worn_slots(worn: &mut WornEquipment) -> [(&'static str, &mut Option<ItemStack>); 7] {
    [
        ("offhand", &mut worn.offhand),
        ("feet", &mut worn.feet),
        ("legs", &mut worn.legs),
        ("chest", &mut worn.chest),
        ("head", &mut worn.head),
        ("body", &mut worn.body),
        ("saddle", &mut worn.saddle),
    ]
}
