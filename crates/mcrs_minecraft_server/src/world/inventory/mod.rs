use bevy_ecs::bundle::Bundle;
use bevy_ecs::component::Component;
use bevy_ecs::entity::Entity;
use mcrs_minecraft_item::{SelectedHotbarSlot, SlotTable, slots};

/// Vanilla hands out container ids as `counter % 100 + 1`.
#[derive(Component, Default, Debug, Clone, Copy)]
pub struct NextContainerId(pub u8);

#[derive(Bundle)]
pub struct PlayerInventoryBundle {
    pub slots: SlotTable,
    pub selected: SelectedHotbarSlot,
    pub next_container: NextContainerId,
}

impl Default for PlayerInventoryBundle {
    fn default() -> Self {
        Self {
            slots: SlotTable::fixed(slots::COUNT),
            selected: SelectedHotbarSlot::default(),
            next_container: NextContainerId::default(),
        }
    }
}

pub fn held_stack(table: &SlotTable, selected: &SelectedHotbarSlot) -> Option<Entity> {
    table.get(slots::held(selected.0))
}
