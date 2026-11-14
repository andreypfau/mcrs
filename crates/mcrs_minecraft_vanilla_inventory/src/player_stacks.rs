use bevy_ecs::system::Query;
use mcrs_minecraft_engine::persisted_registry::PersistedError;
use mcrs_minecraft_engine_item::Slots;
use mcrs_minecraft_engine_item::Stacks;
use mcrs_minecraft_item_stack::ItemStack;
use mcrs_minecraft_item_table::ItemTable;
use mcrs_minecraft_nbt::compound::NbtCompound;
use mcrs_minecraft_nbt::tag::NbtTag;
use mcrs_minecraft_player_slots::ArmourSlot;
use mcrs_minecraft_player_slots::HOTBAR;
use mcrs_minecraft_player_slots::INVENTORY_SLOTS;
use mcrs_minecraft_player_slots::MAIN;
use mcrs_minecraft_player_slots::OFF_HAND;

use crate::PlayerHolders;
use crate::player_file::EQUIPMENT_KEY;
use crate::player_file::INVENTORY_KEY;
use crate::player_file::PlayerFile;
use crate::saved_stack;

pub(crate) const CRAFT_GRID_SLOTS: usize = 4;

pub(crate) const CURSOR_KEY: &str = "mcrs:cursor";
const CRAFT_GRID_KEY: &str = "mcrs:craft_grid";
pub(crate) const CARRIED_KEYS: &[&str] = &[CURSOR_KEY, CRAFT_GRID_KEY];

/// The keys of `equipment` in the player file that hold slots of the player inventory
/// (INV-SAV-001), in the order of a write. The file keeps each other key.
const EQUIPMENT: [EquipmentKey; 5] = [
    EquipmentKey {
        key: "offhand",
        index: OFF_HAND,
    },
    EquipmentKey {
        key: "feet",
        index: ArmourSlot::Feet.index(),
    },
    EquipmentKey {
        key: "legs",
        index: ArmourSlot::Legs.index(),
    },
    EquipmentKey {
        key: "chest",
        index: ArmourSlot::Chest.index(),
    },
    EquipmentKey {
        key: "head",
        index: ArmourSlot::Head.index(),
    },
];

/// A key of `equipment` in the player file and the slot of the player inventory that it holds.
struct EquipmentKey {
    key: &'static str,
    index: u16,
}

/// The stacks of the holders of a player as values, and its selected slot. The craft result has
/// no part: neither the player file nor a carry holds it.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct PlayerStacks {
    /// One value for each slot of INV-SLT-001.
    pub(crate) inventory: Vec<Option<ItemStack>>,
    pub(crate) cursor: Option<ItemStack>,
    pub(crate) craft_grid: [Option<ItemStack>; CRAFT_GRID_SLOTS],
    pub(crate) selected: u8,
}

impl PlayerStacks {
    pub(crate) fn empty() -> Self {
        Self {
            inventory: vec![None; usize::from(INVENTORY_SLOTS)],
            cursor: None,
            craft_grid: Default::default(),
            selected: 0,
        }
    }

    /// The stacks that the holders of a player hold now: `inventory` and, with `holders`, the
    /// cursor and the craft grid.
    pub(crate) fn from_holders(
        inventory: &Slots,
        selected: u8,
        holders: Option<&PlayerHolders>,
        holder_slots: &Query<&Slots>,
        stacks: &mut Stacks,
    ) -> Self {
        let mut values = Self::empty();
        values.selected = selected;
        read_slots(stacks, inventory, &mut values.inventory);
        if let Some(holders) = holders {
            if let Ok(cursor) = holder_slots.get(holders.cursor) {
                values.cursor = cursor.get(0).and_then(|stack| stacks.to_value(stack));
            }
            if let Ok(craft_grid) = holder_slots.get(holders.craft_grid) {
                read_slots(stacks, craft_grid, &mut values.craft_grid);
            }
        }
        values
    }

    /// The stacks that `file` gives, with no filter and no slot limit (INV-SAV-001, INV-SAV-002).
    /// The cursor and the craft grid are empty. A stack that does not load fails the read, so the
    /// next write does not lose it.
    pub(crate) fn read(file: &PlayerFile, table: &ItemTable) -> Result<Self, PersistedError> {
        let mut stacks = Self::empty();
        for entry in &file.inventory {
            let slotted = saved_stack::from_saved_with_slot(INVENTORY_KEY, entry, table)?;
            if !MAIN.contains(&u16::from(slotted.slot)) {
                tracing::error!(
                    slot = slotted.slot,
                    "a stack of the player file has no slot"
                );
                continue;
            }
            if let Some(slot) = stacks.inventory.get_mut(usize::from(slotted.slot)) {
                *slot = Some(slotted.stack);
            }
        }
        for EquipmentKey { key, index } in EQUIPMENT {
            if let Some(value) = file.equipment.get_compound(key)
                && let Some(slot) = stacks.inventory.get_mut(usize::from(index))
            {
                *slot = Some(saved_stack::from_saved(EQUIPMENT_KEY, value, table)?);
            }
        }
        stacks.selected = u8::try_from(file.selected_item_slot)
            .ok()
            .filter(|selected| HOTBAR.contains(&u16::from(*selected)))
            .unwrap_or(0);
        Ok(stacks)
    }

    /// Puts the cursor and the craft grid that `carried` holds into these stacks.
    pub(crate) fn read_carried(
        &mut self,
        carried: &NbtCompound,
        table: &ItemTable,
    ) -> Result<(), PersistedError> {
        if let Some(saved) = carried.get_compound(CURSOR_KEY) {
            self.cursor = Some(saved_stack::from_saved_with_slot(CURSOR_KEY, saved, table)?.stack);
        }
        if let Some(NbtTag::List(craft_grid)) = carried.get(CRAFT_GRID_KEY) {
            for tag in craft_grid {
                let NbtTag::Compound(saved) = tag else {
                    continue;
                };
                let slotted = saved_stack::from_saved_with_slot(CRAFT_GRID_KEY, saved, table)?;
                if let Some(slot) = self.craft_grid.get_mut(usize::from(slotted.slot)) {
                    *slot = Some(slotted.stack);
                }
            }
        }
        Ok(())
    }

    /// Writes the player inventory and the selected slot into `file` (INV-SAV-001).
    pub(crate) fn write(
        &self,
        file: &mut PlayerFile,
        table: &ItemTable,
    ) -> Result<(), PersistedError> {
        file.inventory.clear();
        let main = self.inventory.iter().take(usize::from(MAIN.end));
        for (slot, stack) in (0..=u8::MAX).zip(main) {
            if let Some(stack) = stack {
                file.inventory.push(saved_stack::to_saved_with_slot(
                    INVENTORY_KEY,
                    slot,
                    stack,
                    table,
                )?);
            }
        }
        for EquipmentKey { key, index } in EQUIPMENT {
            if let Some(Some(stack)) = self.inventory.get(usize::from(index)) {
                let saved = saved_stack::to_saved(EQUIPMENT_KEY, stack, table)?;
                file.equipment.put(key, NbtTag::Compound(saved));
            }
        }
        file.selected_item_slot = i32::from(self.selected);
        Ok(())
    }

    /// The cursor and the craft grid as the values that cross to another cell.
    pub(crate) fn write_carried(&self, table: &ItemTable) -> Result<NbtCompound, PersistedError> {
        let mut carried = NbtCompound::new();
        if let Some(cursor) = &self.cursor {
            let saved = saved_stack::to_saved_with_slot(CURSOR_KEY, 0, cursor, table)?;
            carried.put(CURSOR_KEY, NbtTag::Compound(saved));
        }
        let mut craft_grid = Vec::new();
        for (slot, stack) in (0..=u8::MAX).zip(&self.craft_grid) {
            if let Some(stack) = stack {
                let saved = saved_stack::to_saved_with_slot(CRAFT_GRID_KEY, slot, stack, table)?;
                craft_grid.push(NbtTag::Compound(saved));
            }
        }
        if !craft_grid.is_empty() {
            carried.put_list(CRAFT_GRID_KEY, craft_grid);
        }
        Ok(carried)
    }

    /// True when the cursor or the craft grid holds a stack.
    pub(crate) fn has_carried(&self) -> bool {
        self.cursor.is_some() || self.craft_grid.iter().any(Option::is_some)
    }
}

/// True when `key` of `equipment` holds a slot of the player inventory.
pub(crate) fn is_slot_key(key: &str) -> bool {
    EQUIPMENT.iter().any(|equipment| equipment.key == key)
}

fn read_slots(stacks: &mut Stacks, slots: &Slots, values: &mut [Option<ItemStack>]) {
    for (value, stack) in values.iter_mut().zip(slots.iter()) {
        *value = stack.and_then(|stack| stacks.to_value(stack));
    }
}
