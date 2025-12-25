use bevy_ecs::component::Component;
use bevy_ecs::entity::Entity;
use bevy_ecs::prelude::Bundle;
use bevy_ecs::query::WorldQuery;
use bevy_ecs_macros::QueryData;
use derive_more::{Deref, DerefMut};

#[derive(Debug, Clone, Default, Component)]
pub struct PlayerInventorySlots {
    slots: [Option<Entity>; 3 * 9],
}

#[derive(Debug, Clone, Default, Component)]
pub struct PlayerHotbarSlots {
    pub slots: [Option<Entity>; 9],
    pub selected: u8,
}

impl PlayerHotbarSlots {
    pub fn get_selected_slot(&self) -> Option<Entity> {
        self.slots.get(self.selected as usize).copied()?
    }
}

#[derive(Debug, Clone, Default, Component)]
pub struct PlayerOffhandSlot {
    slot: Option<Entity>,
}

#[derive(Debug, Clone, Default, Component)]
pub struct CraftingResultSlot {
    item_stack: Option<Entity>,
}

#[derive(Debug, Clone, Default, Component)]
pub struct PlayerCraftingSlots {
    pub input_slots: [Option<Entity>; 4],
}

#[derive(Debug, Clone, Default, Component)]
pub struct ArmorSlots {
    pub head: Option<Entity>,
    pub chest: Option<Entity>,
    pub legs: Option<Entity>,
    pub feet: Option<Entity>,
}

#[derive(Debug, Clone, Default, Component, Deref, DerefMut)]
pub struct CarriedItem(pub Option<Entity>);

#[derive(Bundle, Default)]
pub struct PlayerInventoryBundle {
    pub result: CraftingResultSlot,
    pub crafting: PlayerCraftingSlots,
    pub armor: ArmorSlots,
    pub inventory: PlayerInventorySlots,
    pub hotbar: PlayerHotbarSlots,
    pub offhand: PlayerOffhandSlot,
    pub carried_item: CarriedItem,
}

#[derive(QueryData)]
pub struct PlayerInventoryQuery {
    pub result: &'static CraftingResultSlot,
    pub crafting: &'static PlayerCraftingSlots,
    pub armor: &'static ArmorSlots,
    pub inventory: &'static PlayerInventorySlots,
    pub hotbar: &'static PlayerHotbarSlots,
    pub offhand: &'static PlayerOffhandSlot,
    pub carried_item: &'static CarriedItem,
}

impl<'w, 's> PlayerInventoryQueryItem<'w, 's> {
    pub fn all_slots(&self) -> Vec<Option<Entity>> {
        let mut slots = Vec::with_capacity(1 + 4 + 4 + 4 * 9 + 1);

        // Crafting result slot
        slots.push(self.result.item_stack);

        // Crafting input slots
        for slot in &self.crafting.input_slots {
            slots.push(*slot);
        }

        // Armor slots
        slots.push(self.armor.head);
        slots.push(self.armor.chest);
        slots.push(self.armor.legs);
        slots.push(self.armor.feet);

        // Main inventory slots
        for slot in &self.inventory.slots {
            slots.push(*slot);
        }

        // Hotbar slots
        for slot in &self.hotbar.slots {
            slots.push(*slot);
        }

        // Offhand slot
        slots.push(self.offhand.slot);

        slots
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Component, Deref, DerefMut)]
pub struct ContainerSeqno(pub u32);
