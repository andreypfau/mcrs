use mcrs_minecraft_client_widget::GuiPoint;
use mcrs_minecraft_menu_geometry::{CreativeGeometry, CreativeHit, CreativeTabKind, VisibleTabs};

#[derive(Clone, Copy, Debug)]
pub(crate) struct CreativeLayout {
    pub(crate) selected: CreativeTabKind,
    pub(crate) visible: VisibleTabs,
}

impl CreativeLayout {
    pub(crate) fn hit(self, width: i32, height: i32, pointer: GuiPoint) -> CreativeHit {
        let window = CreativeGeometry::window(width, height);
        CreativeGeometry::hit(window, pointer, self.selected, self.visible)
    }

    pub(crate) fn searches(self) -> bool {
        self.selected == CreativeTabKind::Search
    }
}
