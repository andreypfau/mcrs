use mcrs_minecraft_inventory_click::Menu;
use mcrs_minecraft_menu::InventoryMenu;
use mcrs_minecraft_menu::MenuRange;
use mcrs_minecraft_menu::MenuSlot;

use crate::click_slots::ClickSlots;

/// The inventory menu over the holders of the player that clicks (INV-MNU-001).
pub(crate) struct ClickMenu<'slots>(pub(crate) &'slots ClickSlots);

impl Menu for ClickMenu<'_> {
    type Slot = MenuSlot;

    fn entry_count(&self) -> u16 {
        InventoryMenu::LEN
    }

    fn slot(&self, index: u16) -> Option<MenuSlot> {
        InventoryMenu::slot(index)
    }

    fn quick_move_destinations(&self, source: u16) -> impl Iterator<Item = u16> {
        let slots = self.0;
        let holds_empty =
            |index: u16| InventoryMenu::slot(index).is_some_and(|slot| slots.holds_empty(slot));
        InventoryMenu::quick_move_destinations(source, None, holds_empty)
            .into_iter()
            .flatten()
            .flat_map(MenuRange::iter)
    }

    fn permits_pick_up_all(&self, index: u16) -> bool {
        InventoryMenu::permits_pick_up_all(index)
    }

    fn permits_drag(&self, index: u16) -> bool {
        InventoryMenu::permits_drag(index)
    }
}
