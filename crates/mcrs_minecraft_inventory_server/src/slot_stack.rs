use bevy_ecs::entity::Entity;
use bevy_ecs::system::Query;
use mcrs_minecraft_engine_item::SlotRef;
use mcrs_minecraft_engine_item::Slots;

pub(crate) fn slot_stack(holders: &Query<&Slots>, slot: SlotRef) -> Option<Entity> {
    holders.get(slot.holder).ok()?.get(usize::from(slot.index))
}
