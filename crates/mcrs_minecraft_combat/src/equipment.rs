use bevy_ecs::prelude::*;
use mcrs_minecraft_item::component::equippable::EquipmentSlot;
use mcrs_minecraft_protocol_game::ProtoStack;
use mcrs_minecraft_registries::Wire;

/// The items that a living entity wears, by slot.
#[derive(Component, Clone, Debug, PartialEq)]
pub struct EntityEquipment {
    slots: [Option<ProtoStack<Wire>>; EquipmentSlot::ALL.len()],
}

impl Default for EntityEquipment {
    fn default() -> Self {
        Self {
            slots: core::array::from_fn(|_| None),
        }
    }
}

impl EntityEquipment {
    #[must_use]
    pub fn new() -> Self {
        Self {
            slots: core::array::from_fn(|_| None),
        }
    }

    pub fn set(&mut self, slot: EquipmentSlot, item: Option<ProtoStack<Wire>>) {
        let idx = slot as usize;
        if let Some(target_slot) = self.slots.get_mut(idx)
            && *target_slot != item
        {
            *target_slot = item;
        }
    }

    #[must_use]
    pub fn get(&self, slot: EquipmentSlot) -> Option<&ProtoStack<Wire>> {
        let idx = slot as usize;
        self.slots.get(idx).and_then(Option::as_ref)
    }

    pub fn iter(&self) -> impl Iterator<Item = (EquipmentSlot, Option<&ProtoStack<Wire>>)> + '_ {
        EquipmentSlot::ALL
            .iter()
            .copied()
            .map(move |slot| (slot, self.get(slot)))
    }
}
