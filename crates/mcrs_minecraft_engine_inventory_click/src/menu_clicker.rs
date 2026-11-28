use mcrs_minecraft_engine_item::RequestId;
use mcrs_minecraft_engine_item::SlotRef;
use mcrs_minecraft_inventory_click::ClickPlayer;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MenuClicker {
    pub player: ClickPlayer,
    pub cursor: SlotRef,
    pub request: RequestId,
}
