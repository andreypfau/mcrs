use bevy_ecs::change_detection::Mut;
use bevy_ecs::message::Message;
use bevy_ecs::message::Messages;
use bevy_ecs::world::EntityWorldMut;

use crate::InSlot;
use crate::SlotRef;

#[derive(Message, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SlotChanged {
    pub slot: SlotRef,
}

pub(crate) fn write_slot_changed(messages: Option<Mut<Messages<SlotChanged>>>, slot: SlotRef) {
    if let Some(mut messages) = messages {
        messages.write(SlotChanged { slot });
    }
}

pub(crate) fn touch(stack: &mut EntityWorldMut) {
    let Some(slot) = stack.get::<InSlot>().copied() else {
        return;
    };
    stack.world_scope(|world| {
        write_slot_changed(
            world.get_resource_mut::<Messages<SlotChanged>>(),
            slot.slot_ref(),
        );
    });
}
