use mcrs_minecraft_client_widget::GuiPoint;
use mcrs_minecraft_client_widget::GuiRect;
use mcrs_minecraft_resource::ResourceLocation;

use crate::CreativeGeometry;
use crate::CreativeTabAlignment;
use crate::CreativeTabKind;
use crate::CreativeTabRow;
use crate::TabSprites;

const PITCH: i32 = 27;
const COLUMNS: i32 = 7;
const SPRITE_WIDTH: i32 = 26;
const SPRITE_HEIGHT: i32 = 32;
const ICON_OFFSET: i32 = 5;
const CLICK_WIDTH: i32 = 27;
const CLICK_HEIGHT: i32 = 33;
const HOVER_INSET: i32 = 2;
const HOVER_WIDTH: i32 = 23;
const HOVER_HEIGHT: i32 = 29;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct CreativeTab {
    pub id: ResourceLocation<'static>,
    pub kind: CreativeTabKind,
    pub row: CreativeTabRow,
    pub column: u8,
    pub alignment: CreativeTabAlignment,
    pub title_key: &'static str,
    pub icon: ResourceLocation<'static>,
    pub sprites: TabSprites,
}

impl CreativeTab {
    #[must_use]
    pub const fn x(&self) -> i32 {
        let column = self.column as i32;
        match self.alignment {
            CreativeTabAlignment::Left => PITCH * column,
            CreativeTabAlignment::Right => CreativeGeometry::WIDTH - PITCH * (COLUMNS - column) + 1,
        }
    }

    #[must_use]
    pub const fn sprite_rect(&self) -> GuiRect {
        let y = match self.row {
            CreativeTabRow::Top => -28,
            CreativeTabRow::Bottom => 132,
        };
        GuiRect::new(self.x(), y, SPRITE_WIDTH, SPRITE_HEIGHT)
    }

    #[must_use]
    pub const fn icon_position(&self) -> GuiPoint {
        let y = match self.row {
            CreativeTabRow::Top => -19,
            CreativeTabRow::Bottom => 139,
        };
        GuiPoint::new(self.x() + ICON_OFFSET, y)
    }

    #[must_use]
    pub const fn click_rect(&self) -> GuiRect {
        let y = match self.row {
            CreativeTabRow::Top => -32,
            CreativeTabRow::Bottom => CreativeGeometry::HEIGHT,
        };
        GuiRect::new(self.x(), y, CLICK_WIDTH, CLICK_HEIGHT)
    }

    #[must_use]
    pub const fn hover_rect(&self) -> GuiRect {
        let y = match self.row {
            CreativeTabRow::Top => -30,
            CreativeTabRow::Bottom => 138,
        };
        GuiRect::new(self.x() + HOVER_INSET, y, HOVER_WIDTH, HOVER_HEIGHT)
    }
}
