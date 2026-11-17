use mcrs_minecraft_engine_item::SlotRef;
use mcrs_minecraft_player_holders::PlayerHolders;

pub(crate) fn cursor_slot(holders: &PlayerHolders) -> SlotRef {
    SlotRef {
        holder: holders.cursor,
        index: 0,
    }
}
