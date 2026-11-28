use bevy_ecs::entity::Entity;
use mcrs_minecraft_engine_item::Request;
use mcrs_minecraft_engine_item::RequestId;
use mcrs_minecraft_engine_item::Selected;
use mcrs_minecraft_engine_item::SlotRef;
use mcrs_minecraft_engine_item::Slots;
use mcrs_minecraft_inventory_player::OFF_HAND;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HandAction {
    DropOne,
    DropStack,
    SwapHands,
}

impl HandAction {
    #[must_use]
    pub fn request(
        self,
        player: Entity,
        slots: &Slots,
        selected: Selected,
        held_count: Option<u8>,
        id: RequestId,
    ) -> Option<Request> {
        let hand = SlotRef {
            holder: player,
            index: u16::from(selected.0),
        };
        match self {
            Self::DropOne => {
                held_count?;
                Some(Request::new(id).drop(hand, 1))
            }
            Self::DropStack => Some(Request::new(id).drop(hand, held_count?)),
            Self::SwapHands => {
                if held_count.is_none() && slots.get(usize::from(OFF_HAND)).is_none() {
                    return None;
                }
                let off_hand = SlotRef {
                    holder: player,
                    index: OFF_HAND,
                };
                Some(Request::new(id).swap(hand, off_hand))
            }
        }
    }
}
