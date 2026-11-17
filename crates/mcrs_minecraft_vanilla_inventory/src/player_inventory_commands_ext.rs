use bevy_ecs::entity::Entity;
use bevy_ecs::system::EntityCommands;
use bevy_ecs::system::In;
use bevy_ecs::system::IntoSystem;
use bevy_ecs::world::EntityWorldMut;
use mcrs_minecraft_engine_item::SlotRef;
use mcrs_minecraft_inventory_file::PlayerItems;
use mcrs_minecraft_item_stack::ItemStack;
use mcrs_minecraft_player_holders::HandAction;
use mcrs_minecraft_player_holders::restore_holders;

use crate::apply_creative_drop::apply_creative_drop;
use crate::apply_creative_set::apply_creative_set;
use crate::apply_selected_slot::apply_selected_slot;
use crate::clear_items::clear_items;
use crate::hand_action::apply_hand_action;

pub trait PlayerInventoryCommandsExt {
    fn hand_action(&mut self, action: HandAction) -> &mut Self;

    fn select_slot(&mut self, slot: u8) -> &mut Self;

    fn creative_set(&mut self, slot: SlotRef, stack: Option<ItemStack>) -> &mut Self;

    fn creative_drop(&mut self, stack: ItemStack) -> &mut Self;

    fn clear_items(&mut self) -> &mut Self;

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

    fn clear_items(&mut self) -> &mut Self {
        queue_system(self, clear_items, |player| player)
    }

    fn restore_items(&mut self, items: PlayerItems) -> &mut Self {
        queue_system(self, restore_holders, move |player| (player, items))
    }
}

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
