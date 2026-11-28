use bevy_ecs::change_detection::Mut;
use bevy_ecs::message::Message;
use bevy_ecs::message::Messages;

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
