use alloc::string::String;
use alloc::string::ToString;
use alloc::vec::Vec;

use bevy_app::App;
use bevy_ecs::component::Component;
use bevy_ecs::entity::Entity;
use bevy_ecs::system::In;
use bevy_ecs::world::World;
use mcrs_minecraft_engine::persisted_registry::CaptureCause;
use mcrs_minecraft_engine::persisted_registry::PersistScope;
use mcrs_minecraft_engine::persisted_registry::PersistedAppExt;
use mcrs_minecraft_engine::persisted_registry::PersistedError;
use mcrs_minecraft_engine::persisted_registry::PersistedForm;
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
use serde::Deserialize;
use serde::Serialize;

use crate::Selected;
use crate::Slots;
use crate::Stacks;
use crate::load_inventory::load_inventory;

/// The slots 0 through 35 of `Inventory`: the hotbar and the main inventory.
const INVENTORY_SLOTS: usize = 36;
const ENDER_SLOTS: u8 = 27;
const HOTBAR_SLOTS: u8 = 9;

const INVENTORY: &str = "Inventory";
const ENDER_ITEMS: &str = "EnderItems";
const EQUIPMENT: &str = "equipment";
const SELECTED_ITEM_SLOT: &str = "SelectedItemSlot";

/// The stacks of the ender chest of a player, as the player file holds them. They have a slot
/// from 0 through 26.
#[derive(Component, Clone, Debug, Default, PartialEq)]
pub struct EnderItems(pub Vec<SlotStack>);

/// The stacks that a living entity wears or holds besides its inventory slots, as the player file
/// holds them.
#[derive(Component, Clone, Debug, Default, PartialEq)]
pub struct WornEquipment {
    pub mainhand: Option<ItemStack>,
    pub offhand: Option<ItemStack>,
    pub feet: Option<ItemStack>,
    pub legs: Option<ItemStack>,
    pub chest: Option<ItemStack>,
    pub head: Option<ItemStack>,
    pub body: Option<ItemStack>,
    pub saddle: Option<ItemStack>,
}

impl WornEquipment {
    /// The slots of a player file: the main hand is the selected slot of `Inventory`, so the
    /// file never holds it.
    fn slots_mut(&mut self) -> [(&'static str, &mut Option<ItemStack>); 7] {
        [
            ("offhand", &mut self.offhand),
            ("feet", &mut self.feet),
            ("legs", &mut self.legs),
            ("chest", &mut self.chest),
            ("head", &mut self.head),
            ("body", &mut self.body),
            ("saddle", &mut self.saddle),
        ]
    }
}

/// `SelectedItemSlot` of the player file: the hotbar slot of the selected stack.
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
            .ok_or_else(|| alloc::format!("no hotbar slot {}", self.0))
    }
}

pub(crate) fn register(app: &mut App) {
    app.register_persisted_codec(
        &[ENDER_ITEMS],
        PersistScope::Persisted,
        capture_ender_items,
        apply_ender_items,
    );
}

pub(crate) fn register_player(app: &mut App) {
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

fn table(world: &World, key: &'static str) -> Result<ItemTable, PersistedError> {
    world
        .get_resource::<ItemTable>()
        .copied()
        .ok_or_else(|| encode_error(key, "the world has no item table"))
}

fn stack_values(In(stacks): In<Vec<Entity>>, mut reader: Stacks) -> Vec<Option<ItemStack>> {
    stacks
        .into_iter()
        .map(|stack| reader.to_value(stack))
        .collect()
}

fn read_stack_values(
    world: &mut World,
    stacks: Vec<Entity>,
    key: &'static str,
) -> Result<Vec<Option<ItemStack>>, PersistedError> {
    world
        .run_system_cached_with(stack_values, stacks)
        .map_err(|error| encode_error(key, error))
}

fn capture_inventory(
    world: &mut World,
    holder: Entity,
    _: CaptureCause,
) -> Result<Option<NbtTag>, PersistedError> {
    let Some(slots) = world.get::<Slots>(holder) else {
        return Ok(None);
    };
    let placed: Vec<(u8, Entity)> = (0..slots.len().min(INVENTORY_SLOTS))
        .filter_map(|index| Some((u8::try_from(index).ok()?, slots.get(index)?)))
        .collect();
    let names = table(world, INVENTORY)?;
    let values = read_stack_values(
        world,
        placed.iter().map(|(_, stack)| *stack).collect(),
        INVENTORY,
    )?;
    let mut list = Vec::new();
    for ((slot, _), stack) in placed.into_iter().zip(values) {
        let Some(stack) = stack else {
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
    names: &ItemTable,
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
    let names = table(world, INVENTORY)?;
    let mut values: Vec<Option<ItemStack>> = alloc::vec![None; INVENTORY_SLOTS];
    for SlotStack { slot, stack } in slot_stacks(tag, &names, INVENTORY)? {
        if let Some(value) = values.get_mut(usize::from(slot)) {
            *value = Some(stack);
        }
    }
    load_inventory(world, holder, values);
    Ok(())
}

fn capture_ender_items(
    world: &mut World,
    holder: Entity,
    _: CaptureCause,
) -> Result<Option<NbtTag>, PersistedError> {
    let Some(items) = world.get::<EnderItems>(holder) else {
        return Ok(None);
    };
    let names = table(world, ENDER_ITEMS)?;
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
    let names = table(world, ENDER_ITEMS)?;
    let mut items = slot_stacks(tag, &names, ENDER_ITEMS)?;
    items.retain(|slotted| slotted.slot < ENDER_SLOTS);
    world
        .get_entity_mut(holder)
        .map_err(|_| PersistedError::NoEntity(holder))?
        .insert(EnderItems(items));
    Ok(())
}

fn capture_equipment(
    world: &mut World,
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
    let names = table(world, EQUIPMENT)?;
    let mut compound = NbtCompound::new();
    for (key, stack) in worn.slots_mut() {
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
    let names = table(world, EQUIPMENT)?;
    let mut worn = WornEquipment::default();
    for (key, entry) in &compound.child_tags {
        let NbtTag::Compound(saved) = entry else {
            return Err(decode_error(EQUIPMENT, "expected an item stack compound"));
        };
        if key == "mainhand" {
            read_saved(saved, &names).map_err(|error| decode_error(EQUIPMENT, error))?;
            continue;
        }
        let Some((_, slot)) = worn.slots_mut().into_iter().find(|(name, _)| name == key) else {
            return Err(decode_error(
                EQUIPMENT,
                alloc::format!("unknown slot `{key}`"),
            ));
        };
        *slot = Some(read_saved(saved, &names).map_err(|error| decode_error(EQUIPMENT, error))?);
    }
    world
        .get_entity_mut(holder)
        .map_err(|_| PersistedError::NoEntity(holder))?
        .insert(worn);
    Ok(())
}
