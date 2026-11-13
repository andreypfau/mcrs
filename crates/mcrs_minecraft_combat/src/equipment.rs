use bevy_ecs::prelude::*;
use mcrs_minecraft_item::component::equippable::EquipmentSlot;

/// The items that a living entity wears, by slot.
#[derive(Component, Clone, Debug, PartialEq, Eq)]
pub struct EntityEquipment {
    slots: [Option<u32>; EquipmentSlot::ALL.len()],
}

impl Default for EntityEquipment {
    fn default() -> Self {
        Self {
            slots: [None; EquipmentSlot::ALL.len()],
        }
    }
}

impl EntityEquipment {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            slots: [None; EquipmentSlot::ALL.len()],
        }
    }

    pub fn set(&mut self, slot: EquipmentSlot, item_id: Option<u32>) {
        let idx = slot as usize;
        if let Some(target_slot) = self.slots.get_mut(idx)
            && *target_slot != item_id
        {
            *target_slot = item_id;
        }
    }

    #[must_use]
    pub fn get(&self, slot: EquipmentSlot) -> Option<u32> {
        let idx = slot as usize;
        self.slots.get(idx).copied().flatten()
    }

    pub fn iter(&self) -> impl Iterator<Item = (EquipmentSlot, Option<u32>)> + '_ {
        EquipmentSlot::ALL
            .iter()
            .copied()
            .map(move |slot| (slot, self.get(slot)))
    }
}
