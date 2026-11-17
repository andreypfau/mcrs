use bevy_ecs::component::Component;
use mcrs_minecraft_inventory_client_view::ClientView;
use mcrs_minecraft_inventory_client_view::StateId;

use crate::close_origin::CloseOrigin;

#[derive(Component, Clone, Debug, PartialEq)]
pub struct MenuSync {
    pub(crate) state_id: StateId,
    pub(crate) view: ClientView,
    pub(crate) container_id: u8,
    pub(crate) close_origin: CloseOrigin,
}

impl MenuSync {
    pub(crate) fn new(container_id: u8, len: u16) -> Self {
        Self {
            state_id: StateId::INITIAL,
            view: ClientView::new(usize::from(len)),
            container_id,
            close_origin: CloseOrigin::Server,
        }
    }

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
        self.container_id
    }
}
