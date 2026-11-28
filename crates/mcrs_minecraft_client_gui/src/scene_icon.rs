use mcrs_minecraft_item_icon::{ItemIcon, Overlays, StackOverlay};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SceneIcon {
    pub icon: ItemIcon,
    pub overlay: StackOverlay,
    pub overlays: Overlays,
}
