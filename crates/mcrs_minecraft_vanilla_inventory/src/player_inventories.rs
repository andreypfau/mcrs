use bevy_ecs::entity::Entity;
use bevy_ecs::system::Query;
use bevy_ecs::system::SystemParam;
use mcrs_minecraft_engine_item::Selected;
use mcrs_minecraft_engine_item::Slots;
use mcrs_minecraft_engine_item::Stacks;
use mcrs_minecraft_inventory_file::PlayerItems;
use mcrs_minecraft_player_holders::PlayerHolders;

use crate::from_holders::from_holders;

#[derive(SystemParam)]
pub struct PlayerInventories<'w, 's> {
    players: Query<
        'w,
        's,
        (
            &'static Slots,
            Option<&'static Selected>,
            &'static PlayerHolders,
        ),
    >,
    holder_slots: Query<'w, 's, &'static Slots>,
    stacks: Stacks<'w, 's>,
}

impl PlayerInventories<'_, '_> {
    pub fn capture(&mut self, player: Entity) -> Option<PlayerItems> {
        let (inventory, selected, holders) = self.players.get(player).ok()?;
        Some(from_holders(
            inventory,
            selected.map_or(0, |selected| selected.0),
            Some(holders),
            |holder| self.holder_slots.get(holder).ok(),
            |stack| self.stacks.to_value(stack),
        ))
    }
}
