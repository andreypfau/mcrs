use bevy_ecs::entity::Entity;
use bevy_ecs::system::Query;
use bevy_ecs::system::SystemParam;
use mcrs_minecraft_engine_item::Selected;
use mcrs_minecraft_engine_item::Slots;
use mcrs_minecraft_engine_item::Stacks;
use mcrs_minecraft_inventory_file::PlayerItems;

use crate::PlayerHolders;
use crate::from_holders::from_holders;

/// Reads the holders of each player and their stacks.
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
    /// The stacks of the player inventory, the cursor and the craft grid of `player` as values,
    /// with its selected slot, or `None` when `player` has no [`PlayerHolders`].
    pub fn capture(&mut self, player: Entity) -> Option<PlayerItems> {
        let (inventory, selected, holders) = self.players.get(player).ok()?;
        Some(from_holders(
            inventory,
            selected.map_or(0, |selected| selected.0),
            Some(holders),
            &self.holder_slots,
            &mut self.stacks,
        ))
    }
}
