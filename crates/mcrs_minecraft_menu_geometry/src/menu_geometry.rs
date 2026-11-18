use mcrs_minecraft_client_widget::GuiPoint;
use mcrs_minecraft_client_widget::GuiRect;
use mcrs_minecraft_container_click::Clicked;
use mcrs_minecraft_inventory_menu::MenuLayout;
use mcrs_minecraft_menu_key::menu_keys::MenuType;
use mcrs_minecraft_resource::ResourceLocation;
use mcrs_minecraft_resource::rl;

use crate::BackgroundPart;
use crate::EntryCorner;
use crate::MenuLabels;
use crate::TextureRect;
use crate::TitlePosition;
use crate::entry_layout::EntryLayout;
use crate::grid::Grid;
use crate::grid::SLOT_PITCH;
use crate::inventory_entries::INVENTORY_ENTRIES;

const ENTRY_SIZE: i32 = 16;
const WINDOW_WIDTH: i32 = 176;
const TITLE: TitlePosition = TitlePosition::Left(GuiPoint::new(8, 6));

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct MenuGeometry {
    width: i32,
    height: i32,
    entries: EntryLayout,
    background: [Option<BackgroundPart>; 2],
    labels: Option<MenuLabels>,
}

impl MenuGeometry {
    pub const INVENTORY: Self = Self {
        width: WINDOW_WIDTH,
        height: 166,
        entries: EntryLayout::Table(&INVENTORY_ENTRIES),
        background: [
            Some(whole_texture(
                rl!("minecraft:textures/gui/container/inventory.png"),
                166,
            )),
            None,
        ],
        labels: None,
    };

    #[must_use]
    pub const fn of(menu: MenuType) -> Option<Self> {
        match menu {
            MenuType::Generic9x1 => Some(generic_rows(1)),
            MenuType::Generic9x2 => Some(generic_rows(2)),
            MenuType::Generic9x3 => Some(generic_rows(3)),
            MenuType::Generic9x4 => Some(generic_rows(4)),
            MenuType::Generic9x5 => Some(generic_rows(5)),
            MenuType::Generic9x6 => Some(generic_rows(6)),
            MenuType::Generic3x3 => Some(generic(
                166,
                Grid {
                    origin: GuiPoint::new(62, 17),
                    columns: 3,
                    rows: 3,
                },
                84,
                rl!("minecraft:textures/gui/container/dispenser.png"),
                TitlePosition::Centred { y: 6 },
            )),
            MenuType::Hopper => Some(generic(
                133,
                Grid {
                    origin: GuiPoint::new(44, 20),
                    columns: 5,
                    rows: 1,
                },
                51,
                rl!("minecraft:textures/gui/container/hopper.png"),
                TITLE,
            )),
            _ => None,
        }
    }

    #[must_use]
    pub fn of_layout(layout: &MenuLayout) -> Option<Self> {
        match layout.kind() {
            None => Some(Self::INVENTORY),
            Some(kind) => Self::of(kind),
        }
    }

    #[must_use]
    pub fn is_inventory(&self) -> bool {
        matches!(self.entries, EntryLayout::Table(_))
    }

    #[must_use]
    pub const fn width(&self) -> i32 {
        self.width
    }

    #[must_use]
    pub const fn height(&self) -> i32 {
        self.height
    }

    #[must_use]
    pub const fn window(&self, screen_width: i32, screen_height: i32) -> GuiPoint {
        GuiPoint::new(
            screen_width.saturating_sub(self.width).div_euclid(2),
            screen_height.saturating_sub(self.height).div_euclid(2),
        )
    }

    #[must_use]
    pub fn entry(&self, index: u16) -> Option<GuiPoint> {
        self.entries.corner(index)
    }

    #[must_use]
    pub fn entry_area(&self, index: u16) -> Option<GuiRect> {
        let corner = self.entries.corner(index)?;
        Some(GuiRect::new(corner.x, corner.y, ENTRY_SIZE, ENTRY_SIZE))
    }

    pub fn entries(&self) -> impl Iterator<Item = EntryCorner> + use<> {
        let entries = self.entries;
        (0..entries.len()).filter_map(move |index| {
            entries
                .corner(index)
                .map(|corner| EntryCorner { index, corner })
        })
    }

    pub fn background(&self) -> impl Iterator<Item = BackgroundPart> + use<> {
        self.background.into_iter().flatten()
    }

    #[must_use]
    pub const fn labels(&self) -> Option<MenuLabels> {
        self.labels
    }

    #[must_use]
    pub fn clicked_at(&self, window: GuiPoint, pointer: GuiPoint) -> Clicked {
        let local = GuiPoint::new(
            pointer.x.saturating_sub(window.x),
            pointer.y.saturating_sub(window.y),
        );
        if !(0..self.width).contains(&local.x) || !(0..self.height).contains(&local.y) {
            return Clicked::Outside;
        }
        (0..self.entries.len())
            .find(|&index| {
                self.entry_area(index)
                    .is_some_and(|area| area.contains_point(local))
            })
            .map_or(Clicked::Nowhere, Clicked::Entry)
    }
}

const fn generic_rows(rows: u16) -> MenuGeometry {
    let container_height = SLOT_PITCH * rows as i32;
    let texture = rl!("minecraft:textures/gui/container/generic_54.png");
    let split = container_height + 17;
    let mut geometry = generic(
        114 + container_height,
        Grid {
            origin: GuiPoint::new(8, 18),
            columns: 9,
            rows,
        },
        container_height + 31,
        texture,
        TITLE,
    );
    geometry.background = [
        Some(BackgroundPart::new(
            texture,
            TextureRect::new(0, 0, WINDOW_WIDTH, split),
            GuiPoint::new(0, 0),
        )),
        Some(BackgroundPart::new(
            texture,
            TextureRect::new(0, 126, WINDOW_WIDTH, 96),
            GuiPoint::new(0, split),
        )),
    ];
    geometry
}

const fn generic(
    height: i32,
    container: Grid,
    player_top: i32,
    texture: ResourceLocation<'static>,
    title: TitlePosition,
) -> MenuGeometry {
    MenuGeometry {
        width: WINDOW_WIDTH,
        height,
        entries: EntryLayout::Generic {
            container,
            player: GuiPoint::new(8, player_top),
        },
        background: [Some(whole_texture(texture, height)), None],
        labels: Some(MenuLabels {
            title,
            player_inventory: GuiPoint::new(8, height - 94),
        }),
    }
}

const fn whole_texture(texture: ResourceLocation<'static>, height: i32) -> BackgroundPart {
    BackgroundPart::new(
        texture,
        TextureRect::new(0, 0, WINDOW_WIDTH, height),
        GuiPoint::new(0, 0),
    )
}
