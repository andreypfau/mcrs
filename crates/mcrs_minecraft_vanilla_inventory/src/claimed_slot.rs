use mcrs_minecraft_inventory_view::SlotView;

/// The view that a click gives for the entry `index` of the menu.
#[derive(Clone, Debug, PartialEq)]
pub struct ClaimedSlot {
    pub index: u16,
    pub view: SlotView,
}
