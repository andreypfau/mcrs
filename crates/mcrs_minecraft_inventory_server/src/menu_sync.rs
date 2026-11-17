use bevy_ecs::component::Component;
use mcrs_minecraft_inventory_client_view::ClientView;
use mcrs_minecraft_inventory_client_view::StateId;
use mcrs_minecraft_inventory_menu::InventoryMenu;

#[derive(Component, Clone, Debug, PartialEq)]
pub struct MenuSync {
    pub(crate) state_id: StateId,
    pub(crate) view: ClientView,
}

impl MenuSync {
    #[must_use]
    pub fn state_id(&self) -> StateId {
        self.state_id
    }

    #[must_use]
    pub fn view(&self) -> &ClientView {
        &self.view
    }

    #[must_use]
    pub fn container_id(&self) -> u8 {
        InventoryMenu::CONTAINER_ID
    }
}

impl Default for MenuSync {
    fn default() -> Self {
        Self {
            state_id: StateId::INITIAL,
            view: ClientView::new(usize::from(InventoryMenu::LEN)),
        }
    }
}
