use mcrs_minecraft_engine_inventory_menu::MenuParts;
use mcrs_minecraft_engine_item::SlotRef;
use mcrs_minecraft_inventory_menu::MenuLayout;

#[derive(Clone, Copy, Debug)]
pub(crate) struct MenuSlots<'menu> {
    pub(crate) layout: &'menu MenuLayout,
    pub(crate) parts: &'menu MenuParts,
}

impl MenuSlots<'_> {
    pub(crate) fn len(&self) -> u16 {
        self.layout.len()
    }

    pub(crate) fn slot(&self, index: u16) -> Option<SlotRef> {
        self.parts.slot(self.layout.entry(index)?)
    }
}
