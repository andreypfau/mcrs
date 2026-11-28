use crate::MenuPacket;

const INVENTORY_MENU: i32 = 0;

#[derive(Default)]
pub(crate) struct MenuView {
    inventory_state_id: i32,
    open: Option<OpenMenu>,
}

struct OpenMenu {
    container: i32,
    state_id: i32,
}

impl MenuView {
    pub(crate) fn container(&self) -> i32 {
        self.open
            .as_ref()
            .map_or(INVENTORY_MENU, |menu| menu.container)
    }

    pub(crate) fn state_id(&self, container: i32) -> i32 {
        if container == INVENTORY_MENU {
            return self.inventory_state_id;
        }
        match &self.open {
            Some(menu) if menu.container == container => menu.state_id,
            _ => 0,
        }
    }

    pub(crate) fn close(&mut self) {
        self.open = None;
    }

    pub(crate) fn observe(&mut self, packet: &MenuPacket) {
        match *packet {
            MenuPacket::OpenScreen { container, .. } => {
                self.open = Some(OpenMenu {
                    container,
                    state_id: 0,
                });
            }
            MenuPacket::ContainerClose { .. } => self.close(),
            MenuPacket::ContainerSetContent {
                container,
                state_id,
                ..
            }
            | MenuPacket::ContainerSetSlot {
                container,
                state_id,
                ..
            } => {
                if container == INVENTORY_MENU {
                    self.inventory_state_id = state_id;
                } else if let Some(menu) = &mut self.open
                    && menu.container == container
                {
                    menu.state_id = state_id;
                }
            }
            MenuPacket::ContainerSetData { .. }
            | MenuPacket::SetCursorItem { .. }
            | MenuPacket::SetPlayerInventory { .. }
            | MenuPacket::SetHeldSlot { .. } => {}
        }
    }
}
