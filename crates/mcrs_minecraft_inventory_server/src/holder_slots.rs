use bevy_ecs::entity::Entity;
use bevy_ecs::system::Query;
use mcrs_minecraft_engine_item::Slots;
use mcrs_minecraft_inventory_player::Holder;

use mcrs_minecraft_vanilla_inventory::PlayerHolders;

pub(crate) struct HolderSlots<'slots> {
    inventory: &'slots Slots,
    cursor: Option<&'slots Slots>,
    craft_grid: Option<&'slots Slots>,
    craft_result: Option<&'slots Slots>,
}

impl<'slots> HolderSlots<'slots> {
    pub(crate) fn read(
        player: Entity,
        inventory: &'slots Slots,
        holders: &PlayerHolders,
        holder_slots: &'slots Query<&Slots>,
    ) -> Self {
        let slots_of = |holder| {
            holder_slots
                .get(holder_entity(holders, player, holder))
                .ok()
        };
        Self {
            inventory,
            cursor: slots_of(Holder::Cursor),
            craft_grid: slots_of(Holder::CraftGrid),
            craft_result: slots_of(Holder::CraftResult),
        }
    }

    pub(crate) fn stack(&self, holder: Holder, index: u16) -> Option<Entity> {
        let slots = match holder {
            Holder::Inventory => Some(self.inventory),
            Holder::Cursor => self.cursor,
            Holder::CraftGrid => self.craft_grid,
            Holder::CraftResult => self.craft_result,
        };
        slots?.get(usize::from(index))
    }
}

pub(crate) fn holder_entity(holders: &PlayerHolders, player: Entity, holder: Holder) -> Entity {
    match holder {
        Holder::Inventory => player,
        Holder::Cursor => holders.cursor,
        Holder::CraftGrid => holders.craft_grid,
        Holder::CraftResult => holders.craft_result,
    }
}
