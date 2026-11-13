use mcrs_minecraft_item_stack::ItemStack;
use mcrs_minecraft_item_table::ItemTable;
use mcrs_minecraft_nbt::compound::NbtCompound;
use mcrs_minecraft_nbt::tag::NbtTag;
use mcrs_minecraft_player_slots::ArmourSlot;
use mcrs_minecraft_player_slots::HOTBAR;
use mcrs_minecraft_player_slots::INVENTORY_SLOTS;
use mcrs_minecraft_player_slots::MAIN;
use mcrs_minecraft_player_slots::OFF_HAND;

use crate::player_file::PlayerFile;
use crate::saved_stack;

pub(crate) const CRAFT_GRID_SLOTS: usize = 4;

pub(crate) const CURSOR_KEY: &str = "mcrs:cursor";
const CRAFT_GRID_KEY: &str = "mcrs:craft_grid";
pub(crate) const CARRIED_KEYS: &[&str] = &[CURSOR_KEY, CRAFT_GRID_KEY];

/// The keys of `equipment` in the player file that hold slots of the player inventory
/// (INV-SAV-001), in the order of a write. The file keeps each other key.
const EQUIPMENT: [(&str, u16); 5] = [
    ("offhand", OFF_HAND),
    ("feet", ArmourSlot::Feet.index()),
    ("legs", ArmourSlot::Legs.index()),
    ("chest", ArmourSlot::Chest.index()),
    ("head", ArmourSlot::Head.index()),
];

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

    /// The stacks that `file` gives, with no filter and no slot limit (INV-SAV-001, INV-SAV-002).
    /// The cursor and the craft grid are empty.
    pub(crate) fn read(file: &PlayerFile, table: &ItemTable) -> Self {
        let mut stacks = Self::empty();
        for entry in &file.inventory {
            let Some(slotted) = saved_stack::from_saved_with_slot(entry, table) else {
                continue;
            };
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
        for (key, index) in EQUIPMENT {
            if let Some(value) = file.equipment.get_compound(key)
                && let Some(slot) = stacks.inventory.get_mut(usize::from(index))
            {
                *slot = saved_stack::from_saved(value, table);
            }
        }
        stacks.selected = u8::try_from(file.selected_item_slot)
            .ok()
            .filter(|selected| HOTBAR.contains(&u16::from(*selected)))
            .unwrap_or(0);
        stacks
    }

    /// Puts the cursor and the craft grid that `carried` holds into these stacks.
    pub(crate) fn read_carried(&mut self, carried: &NbtCompound, table: &ItemTable) {
        self.cursor = carried
            .get_compound(CURSOR_KEY)
            .and_then(|saved| saved_stack::from_saved_with_slot(saved, table))
            .map(|slotted| slotted.stack);
        if let Some(NbtTag::List(craft_grid)) = carried.get(CRAFT_GRID_KEY) {
            for tag in craft_grid {
                if let NbtTag::Compound(saved) = tag
                    && let Some(slotted) = saved_stack::from_saved_with_slot(saved, table)
                    && let Some(slot) = self.craft_grid.get_mut(usize::from(slotted.slot))
                {
                    *slot = Some(slotted.stack);
                }
            }
        }
    }

    /// Writes the player inventory and the selected slot into `file` (INV-SAV-001).
    pub(crate) fn write(&self, file: &mut PlayerFile, table: &ItemTable) {
        file.inventory.clear();
        let main = self.inventory.iter().take(usize::from(MAIN.end));
        for (slot, stack) in (0..=u8::MAX).zip(main) {
            if let Some(stack) = stack
                && let Some(saved) = saved_stack::to_saved_with_slot(slot, stack, table)
            {
                file.inventory.push(saved);
            }
        }
        for (key, index) in EQUIPMENT {
            let saved = self
                .inventory
                .get(usize::from(index))
                .and_then(Option::as_ref)
                .and_then(|stack| saved_stack::to_saved(stack, table));
            if let Some(saved) = saved {
                file.equipment.put(key, NbtTag::Compound(saved));
            }
        }
        file.selected_item_slot = i32::from(self.selected);
    }

    /// The cursor and the craft grid as the values that cross to another cell.
    pub(crate) fn write_carried(&self, table: &ItemTable) -> NbtCompound {
        let mut carried = NbtCompound::new();
        if let Some(cursor) = &self.cursor
            && let Some(saved) = saved_stack::to_saved_with_slot(0, cursor, table)
        {
            carried.put(CURSOR_KEY, NbtTag::Compound(saved));
        }
        let craft_grid: Vec<NbtTag> = (0..=u8::MAX)
            .zip(&self.craft_grid)
            .filter_map(|(slot, stack)| {
                saved_stack::to_saved_with_slot(slot, stack.as_ref()?, table)
            })
            .map(NbtTag::Compound)
            .collect();
        if !craft_grid.is_empty() {
            carried.put_list(CRAFT_GRID_KEY, craft_grid);
        }
        carried
    }

    /// True when the cursor or the craft grid holds a stack.
    pub(crate) fn has_carried(&self) -> bool {
        self.cursor.is_some() || self.craft_grid.iter().any(Option::is_some)
    }
}

/// True when `key` of `equipment` holds a slot of the player inventory.
pub(crate) fn is_slot_key(key: &str) -> bool {
    EQUIPMENT.iter().any(|(slot_key, _)| *slot_key == key)
}
