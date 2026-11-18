use mcrs_minecraft_client_widget::GuiPoint;
use mcrs_minecraft_resource::ResourceLocation;
use mcrs_minecraft_resource::rl;

use crate::BackgroundPart;
use crate::CreativeGeometry;
use crate::TextureRect;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CreativeBackground {
    Items,
    ItemSearch,
    Inventory,
}

impl CreativeBackground {
    #[must_use]
    pub const fn texture(self) -> ResourceLocation<'static> {
        match self {
            Self::Items => rl!("minecraft:textures/gui/container/creative_inventory/tab_items.png"),
            Self::ItemSearch => {
                rl!("minecraft:textures/gui/container/creative_inventory/tab_item_search.png")
            }
            Self::Inventory => {
                rl!("minecraft:textures/gui/container/creative_inventory/tab_inventory.png")
            }
        }
    }

    #[must_use]
    pub const fn part(self) -> BackgroundPart {
        BackgroundPart::new(
            self.texture(),
            TextureRect::new(0, 0, CreativeGeometry::WIDTH, CreativeGeometry::HEIGHT),
            GuiPoint::new(0, 0),
        )
    }
}
