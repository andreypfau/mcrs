use bevy_ecs::change_detection::Mut;
use bevy_ecs::message::Message;
use bevy_ecs::message::Messages;
use bevy_ecs::world::EntityWorldMut;

use crate::InSlot;
use crate::SlotRef;

/// The slot holds a different stack, a different count or a different component value than
/// before the command or the despawn that wrote it. One slot can have more than one message in a
/// tick; a consumer removes the repeats.
#[derive(Message, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SlotChanged {
    /// The slot whose content changed.
    pub slot: SlotRef,
}

/// Writes nothing in a world without [`ItemPlugin`](crate::ItemPlugin), as a plain test world.
pub(crate) fn write_slot_changed(messages: Option<Mut<Messages<SlotChanged>>>, slot: SlotRef) {
    if let Some(mut messages) = messages {
        messages.write(SlotChanged { slot });
    }
}

/// Writes [`SlotChanged`] for the slot of the stack, when it has one.
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
