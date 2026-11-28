use bevy::math::DVec2;
use mcrs_minecraft_client_widget::GuiRect;
use mcrs_minecraft_menu_geometry::CreativeGeometry;

use crate::creative_screen_draw;
use crate::gui_pointer::GuiPointer;
use crate::gui_size::GuiSize;

#[derive(Clone, Copy, Debug)]
pub(crate) struct CreativePointer {
    pub(crate) top: i32,
    pub(crate) search_box: GuiRect,
    pub(crate) mouse: DVec2,
}

pub(crate) fn creative_pointer(size: GuiSize, pointer: GuiPointer) -> CreativePointer {
    let corner = CreativeGeometry::window(size.width, size.height);
    CreativePointer {
        top: corner.y,
        search_box: creative_screen_draw::offset(corner, CreativeGeometry::SEARCH_BOX),
        mouse: pointer.at(),
    }
}
