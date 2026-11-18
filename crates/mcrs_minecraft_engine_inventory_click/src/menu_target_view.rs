use bevy_ecs::entity::Entity;
use mcrs_minecraft_engine_inventory_menu::MenuButtons;
use mcrs_minecraft_engine_inventory_menu::MenuParts;
use mcrs_minecraft_inventory_click::DragEntry;
use mcrs_minecraft_inventory_click::DragKind;
use mcrs_minecraft_inventory_click::DragSplit;
use mcrs_minecraft_inventory_click::Menu;
use mcrs_minecraft_inventory_click::Slots;
use mcrs_minecraft_inventory_menu::MenuLayout;

use crate::click_menu::ClickMenu;
use crate::click_slots::ClickSlots;

pub struct MenuTargetView<'w> {
    pub(crate) layout: &'w MenuLayout,
    pub(crate) parts: &'w MenuParts,
    pub(crate) buttons: MenuButtons,
    pub(crate) slots: ClickSlots<'w>,
}

impl MenuTargetView<'_> {
    fn menu(&self) -> ClickMenu<'_> {
        ClickMenu {
            layout: self.layout,
            parts: self.parts,
            buttons: self.buttons,
            slots: &self.slots,
        }
    }

    #[must_use]
    pub fn joins_drag(&self, index: u16) -> Option<DragEntry> {
        mcrs_minecraft_inventory_click::joins_drag(&self.menu(), &self.slots, index)
    }

    #[must_use]
    pub fn stays_in_drag(&self, index: u16) -> Option<DragEntry> {
        mcrs_minecraft_inventory_click::stays_in_drag(&self.menu(), &self.slots, index)
    }

    #[must_use]
    pub fn drag_split(&self, kind: DragKind, size: u32) -> Option<DragSplit> {
        let carried = self.slots.load(self.slots.cursor())?.stack?;
        Some(DragSplit::from_carried(kind, &carried, size))
    }

    #[must_use]
    pub fn permits_pick_up_all(&self, index: u16) -> bool {
        self.menu().permits_pick_up_all(index)
    }

    #[must_use]
    pub fn entry_count(&self) -> u16 {
        self.menu().entry_count()
    }

    #[must_use]
    pub fn shift_double_click_moves(
        &self,
        target: u16,
        index: u16,
        alike: impl FnOnce(Entity) -> bool,
    ) -> bool {
        mcrs_minecraft_inventory_click::shift_double_click_moves(
            &self.menu(),
            &self.slots,
            target,
            index,
            |stack| alike(stack.entity),
        )
    }
}
