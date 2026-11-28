use mcrs_minecraft_inventory_client_view::SlotView;

#[derive(Clone, Debug, PartialEq)]
pub struct ClaimedSlot {
    pub index: u16,
    pub view: SlotView,
}
