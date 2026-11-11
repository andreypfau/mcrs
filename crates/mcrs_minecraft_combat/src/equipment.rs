use bevy_ecs::message::{Message, MessageWriter};
use bevy_ecs::prelude::*;
use mcrs_minecraft_item::component::equippable::EquipmentSlot;

/// Component tracking equipped items on a living entity (#115).
#[derive(Component, Clone, Debug, PartialEq, Eq)]
pub struct EntityEquipment {
    slots: [Option<u32>; EquipmentSlot::ALL.len()],
    changed: bool,
}

impl Default for EntityEquipment {
    fn default() -> Self {
        Self {
            slots: [None; EquipmentSlot::ALL.len()],
            changed: false,
        }
    }
}

impl EntityEquipment {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            slots: [None; EquipmentSlot::ALL.len()],
            changed: false,
        }
    }

    pub fn set(&mut self, slot: EquipmentSlot, item_id: Option<u32>) {
        let idx = slot as usize;
        if let Some(target_slot) = self.slots.get_mut(idx)
            && *target_slot != item_id
        {
            *target_slot = item_id;
            self.changed = true;
        }
    }

    #[must_use]
    pub fn get(&self, slot: EquipmentSlot) -> Option<u32> {
        let idx = slot as usize;
        self.slots.get(idx).copied().flatten()
    }

    #[must_use]
    pub const fn has_changed(&self) -> bool {
        self.changed
    }

    pub fn clear_changed(&mut self) {
        self.changed = false;
    }

    pub fn iter(&self) -> impl Iterator<Item = (EquipmentSlot, Option<u32>)> + '_ {
        EquipmentSlot::ALL.iter().copied().map(move |slot| (slot, self.get(slot)))
    }
}

/// Message emitted when an entity's equipment changes.
#[derive(Message, Clone, Copy, Debug, PartialEq, Eq)]
pub struct EquipmentChangedEvent {
    pub entity: Entity,
    pub slot: EquipmentSlot,
    pub item_id: Option<u32>,
}

/// System that detects changes in `EntityEquipment` and emits `EquipmentChangedEvent`.
pub fn track_equipment_changes(
    mut query: Query<(Entity, &mut EntityEquipment)>,
    mut events: MessageWriter<EquipmentChangedEvent>,
) {
    for (entity, mut equip) in &mut query {
        if equip.has_changed() {
            for (slot, item) in equip.iter() {
                events.write(EquipmentChangedEvent {
                    entity,
                    slot,
                    item_id: item,
                });
            }
            equip.clear_changed();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_equipment_slot_set_and_get() {
        let mut equip = EntityEquipment::new();
        assert_eq!(equip.get(EquipmentSlot::MainHand), None);
        assert!(!equip.has_changed());

        equip.set(EquipmentSlot::MainHand, Some(42));
        assert_eq!(equip.get(EquipmentSlot::MainHand), Some(42));
        assert!(equip.has_changed());

        equip.clear_changed();
        assert!(!equip.has_changed());
    }
}
