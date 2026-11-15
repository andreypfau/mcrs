use bevy_ecs::entity::Entity;
use bevy_ecs::system::EntityCommands;
use bevy_ecs::system::In;
use bevy_ecs::system::IntoSystem;
use bevy_ecs::world::EntityWorldMut;
use mcrs_minecraft_engine_item::SlotRef;
use mcrs_minecraft_inventory_file::PlayerItems;
use mcrs_minecraft_item_stack::ItemStack;

use crate::HandAction;
use crate::apply_close::apply_close;
use crate::apply_creative_drop::apply_creative_drop;
use crate::apply_creative_set::apply_creative_set;
use crate::apply_selected_slot::apply_selected_slot;
use crate::clear_items::clear_items;
use crate::hand_action::apply_hand_action;
use crate::restore_holders::restore_holders;

/// The inventory actions of a player, for the [`EntityCommands`] of that player. Each one commits
/// its request when the command applies, so a later command reads the slots that it leaves.
pub trait PlayerInventoryCommandsExt {
    /// Makes the request of a drop key or of the hand swap (INV-SEL-004, INV-SEL-005). A
    /// spectator and an empty hand give no request.
    fn hand_action(&mut self, action: HandAction) -> &mut Self;

    /// Makes `slot` the selected slot when it is in `0..=8`, and ignores each other value
    /// (INV-SEL-002).
    fn select_slot(&mut self, slot: u8) -> &mut Self;

    /// Sets `slot` to `stack` by a request (INV-CRE-004). It does nothing for a player that is
    /// not in creative mode, or for a count above the maximum stack size (INV-CRE-003).
    fn creative_set(&mut self, slot: SlotRef, stack: Option<ItemStack>) -> &mut Self;

    /// Throws `stack` from the player (INV-CRE-003). It does nothing for a player that is not in
    /// creative mode, or for a count above the maximum stack size.
    fn creative_drop(&mut self, stack: ItemStack) -> &mut Self;

    /// Moves the cursor and the craft grid into the player inventory by one request, or drops
    /// what finds no room (INV-CLS-001).
    // chisle: a close is a method of the player until the menu crate has an event for a closed
    // menu, which an observer of this crate then answers.
    fn close_menu(&mut self) -> &mut Self;

    /// Despawns each stack of the four holders of the player, the cursor included.
    fn clear_items(&mut self) -> &mut Self;

    /// Replaces the stacks of the player inventory, the cursor and the craft grid, and sets the
    /// selected slot. It triggers [`PlayerItemsRestored`](crate::PlayerItemsRestored).
    fn restore_items(&mut self, items: PlayerItems) -> &mut Self;
}

impl PlayerInventoryCommandsExt for EntityCommands<'_> {
    fn hand_action(&mut self, action: HandAction) -> &mut Self {
        queue_system(self, apply_hand_action, move |player| (player, action))
    }

    fn select_slot(&mut self, slot: u8) -> &mut Self {
        queue_system(self, apply_selected_slot, move |player| (player, slot))
    }

    fn creative_set(&mut self, slot: SlotRef, stack: Option<ItemStack>) -> &mut Self {
        queue_system(self, apply_creative_set, move |player| {
            (player, slot, stack)
        })
    }

    fn creative_drop(&mut self, stack: ItemStack) -> &mut Self {
        queue_system(self, apply_creative_drop, move |player| (player, stack))
    }

    fn close_menu(&mut self) -> &mut Self {
        queue_system(self, apply_close, |player| player)
    }

    fn clear_items(&mut self) -> &mut Self {
        queue_system(self, clear_items, |player| player)
    }

    fn restore_items(&mut self, items: PlayerItems) -> &mut Self {
        queue_system(self, restore_holders, move |player| (player, items))
    }
}

/// Runs `system` on the player with the input that `input` makes from it, when the command
/// applies.
fn queue_system<'commands, 'world, T, M, S>(
    commands: &'commands mut EntityCommands<'world>,
    system: S,
    input: impl FnOnce(Entity) -> T + Send + 'static,
) -> &'commands mut EntityCommands<'world>
where
    T: Send + 'static,
    S: IntoSystem<In<T>, (), M> + Send + 'static,
{
    commands.queue_silenced(move |entity: EntityWorldMut| {
        let player = entity.id();
        let applied = entity
            .into_world_mut()
            .run_system_cached_with(system, input(player));
        if let Err(error) = applied {
            tracing::warn!(%error, "an inventory command did not apply");
        }
    })
}
