use bevy_ecs::component::Component;
use mcrs_minecraft_inventory_view::ClientView;
use mcrs_minecraft_inventory_view::StateId;
use mcrs_minecraft_menu::InventoryMenu;

/// The state id of the inventory menu of a player and its client view: what the server sent last
/// for each entry and for the cursor (INV-SYN-001, INV-SYN-002).
#[derive(Component, Clone, Debug, PartialEq)]
pub struct InventoryMenuSync {
    pub(crate) state_id: StateId,
    pub(crate) view: ClientView,
}

impl InventoryMenuSync {
    /// The state id that the last content or slot packet of the inventory menu carried.
    #[must_use]
    pub fn state_id(&self) -> StateId {
        self.state_id
    }

    /// The client view of each entry of the inventory menu, in the order of the menu index, and
    /// of the cursor.
    #[must_use]
    pub fn view(&self) -> &ClientView {
        &self.view
    }
}

impl Default for InventoryMenuSync {
    fn default() -> Self {
        Self {
            state_id: StateId::INITIAL,
            view: ClientView::new(usize::from(InventoryMenu::LEN)),
        }
    }
}
