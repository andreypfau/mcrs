use mcrs_minecraft_client_widget::GuiPoint;
use mcrs_minecraft_item_stack::ItemStack;
use mcrs_minecraft_menu_geometry::{CreativeGeometry, CreativeHit, CreativeTabKind};

use crate::creative_screen_state::CreativeScreenState;
use crate::creative_tab_content::CreativeTabContent;

#[derive(Clone, Copy)]
pub(crate) struct CreativeGrid<'a> {
    pub(crate) state: &'a CreativeScreenState,
    pub(crate) content: &'a CreativeTabContent,
}

impl<'a> CreativeGrid<'a> {
    pub(crate) fn stack(self, slot: u16) -> Option<&'a ItemStack> {
        self.state.grid_stack(slot, self.content)
    }

    pub(crate) fn hit(self, width: i32, height: i32, pointer: GuiPoint) -> CreativeHit {
        let window = CreativeGeometry::window(width, height);
        let selected = self.state.selected_tab().kind;
        CreativeGeometry::hit(window, pointer, selected, self.content.visible_tabs())
    }

    pub(crate) fn searches(self) -> bool {
        self.state.selected_tab().kind == CreativeTabKind::Search
    }

    pub(crate) fn can_scroll(self) -> bool {
        self.state.can_scroll(self.content)
    }
}
