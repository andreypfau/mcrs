use mcrs_minecraft_engine_inventory_menu::MenuButtons;
use mcrs_minecraft_engine_item::SlotRef;
use mcrs_minecraft_inventory_click::Menu;
use mcrs_minecraft_inventory_menu::RouteWhen;

use crate::menu_slots::MenuSlots;

pub(crate) struct ClickMenu<'menu> {
    pub(crate) slots: MenuSlots<'menu>,
    pub(crate) buttons: MenuButtons,
}

impl Menu for ClickMenu<'_> {
    type Slot = SlotRef;

    fn entry_count(&self) -> u16 {
        self.slots.len()
    }

    fn slot(&self, index: u16) -> Option<SlotRef> {
        self.slots.slot(index)
    }

    fn quick_move_destinations(&self, source: u16) -> impl Iterator<Item = u16> {
        let buttons = self.buttons;
        let route = (self.slots.layout.routes(source)).find(|route| holds(route.when));
        route
            .into_iter()
            .flat_map(|route| route.to.iter())
            .filter(move |index| !buttons.contains(*index))
    }

    fn permits_pick_up_all(&self, index: u16) -> bool {
        self.slots.layout.permits_pick_up_all(index) && !self.buttons.contains(index)
    }

    fn permits_drag(&self, index: u16) -> bool {
        self.slots.layout.permits_drag(index) && !self.buttons.contains(index)
    }
}

fn holds(when: RouteWhen) -> bool {
    match when {
        RouteWhen::Always => true,
        RouteWhen::Wears { .. } => false,
    }
}
