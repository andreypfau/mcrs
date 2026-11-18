use mcrs_minecraft_item_stack::ItemStack;

use crate::creative_layout::CreativeLayout;
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

    pub(crate) fn layout(self) -> CreativeLayout {
        CreativeLayout {
            selected: self.state.selected_tab().kind,
            visible: self.content.visible_tabs(),
        }
    }

    pub(crate) fn can_scroll(self) -> bool {
        self.state.can_scroll(self.content)
    }
}
