use mcrs_minecraft_engine_inventory_menu::MenuButtons;
use mcrs_minecraft_engine_inventory_menu::MenuParts;
use mcrs_minecraft_engine_item::SlotRef;
use mcrs_minecraft_inventory_click::Menu;
use mcrs_minecraft_inventory_menu::MenuLayout;

use crate::click_slots::ClickSlots;

#[derive(Clone, Copy)]
pub(crate) struct ClickMenu<'menu> {
    pub(crate) layout: &'menu MenuLayout,
    pub(crate) parts: &'menu MenuParts,
    pub(crate) buttons: MenuButtons,
    pub(crate) slots: &'menu ClickSlots<'menu>,
}

impl Menu for ClickMenu<'_> {
    type Slot = SlotRef;

    fn entry_count(&self) -> u16 {
        self.layout.len()
    }

    fn slot(&self, index: u16) -> Option<SlotRef> {
        self.parts.slot(self.layout.entry(index)?)
    }

    fn quick_move_destinations(
        &self,
        source: u16,
        holds_empty: impl Fn(u16) -> bool,
    ) -> impl Iterator<Item = u16> {
        let buttons = self.buttons;
        let worn = self.slot(source).and_then(|slot| self.slots.worn(slot));
        let route = (self.layout.routes(source)).find(|route| route.when.holds(worn, &holds_empty));
        route
            .into_iter()
            .flat_map(|route| route.to.iter())
            .filter(move |index| !buttons.contains(*index))
    }

    fn permits_pick_up_all(&self, index: u16) -> bool {
        self.layout.permits_pick_up_all(index) && !self.buttons.contains(index)
    }

    fn permits_drag(&self, index: u16) -> bool {
        self.layout.permits_drag(index) && !self.buttons.contains(index)
    }
}
