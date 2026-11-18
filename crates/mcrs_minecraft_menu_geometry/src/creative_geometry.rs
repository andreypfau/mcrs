use mcrs_minecraft_client_widget::GuiPoint;
use mcrs_minecraft_client_widget::GuiRect;
use mcrs_minecraft_resource::ResourceLocation;
use mcrs_minecraft_resource::rl;

use crate::CreativeHit;
use crate::CreativeTab;
use crate::CreativeTabIndex;
use crate::CreativeTabKind;
use crate::VisibleTabs;
use crate::grid::Grid;

const ENTRY_SIZE: i32 = 16;
const GRID_COLUMNS: u16 = 9;
const GRID_ROWS: u16 = 5;
const GRID_SLOTS: u16 = GRID_COLUMNS * GRID_ROWS;
pub(crate) const HOTBAR_MENU_START: u16 = 36;
const HEAD_MENU_INDEX: u16 = 5;
const CHEST_MENU_INDEX: u16 = 6;
const LEGS_MENU_INDEX: u16 = 7;
const FEET_MENU_INDEX: u16 = 8;
const MAIN_MENU_START: u16 = 9;
const OFF_HAND_MENU_INDEX: u16 = 45;
const SCROLL_TRAVEL: f64 = 95.0;
const DRAG_OFFSET: f64 = 25.5;
const DRAG_TRAVEL: f64 = 97.0;

const GRID: Grid = Grid {
    origin: GuiPoint::new(9, 18),
    columns: GRID_COLUMNS,
    rows: GRID_ROWS,
};
const HOTBAR: Grid = Grid {
    origin: GuiPoint::new(9, 112),
    columns: GRID_COLUMNS,
    rows: 1,
};
const MAIN_INVENTORY: Grid = Grid {
    origin: GuiPoint::new(9, 54),
    columns: GRID_COLUMNS,
    rows: 3,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct CreativeGeometry;

impl CreativeGeometry {
    pub const WIDTH: i32 = 195;
    pub const HEIGHT: i32 = 136;
    pub const TITLE: GuiPoint = GuiPoint::new(8, 6);
    pub const SEARCH_BOX: GuiRect = GuiRect::new(82, 6, 80, 9);
    pub const DESTROY_SLOT: GuiPoint = GuiPoint::new(173, 112);
    pub const SCROLL_BAR: GuiRect = GuiRect::new(175, 18, 14, 112);
    pub const SCROLLER: ResourceLocation<'static> =
        rl!("minecraft:container/creative_inventory/scroller");
    pub const SCROLLER_DISABLED: ResourceLocation<'static> =
        rl!("minecraft:container/creative_inventory/scroller_disabled");

    #[must_use]
    pub const fn window(screen_width: i32, screen_height: i32) -> GuiPoint {
        GuiPoint::new(
            screen_width.saturating_sub(Self::WIDTH).div_euclid(2),
            screen_height.saturating_sub(Self::HEIGHT).div_euclid(2),
        )
    }

    #[must_use]
    pub fn grid_slot(index: u16) -> Option<GuiPoint> {
        GRID.corner(index)
    }

    #[must_use]
    pub fn content_index(slot: u16, first_row: i32) -> Option<usize> {
        (slot < GRID_SLOTS).then_some(())?;
        let first_row = usize::try_from(first_row).ok()?;
        let columns = usize::from(GRID_COLUMNS);
        first_row
            .checked_mul(columns)?
            .checked_add(usize::from(slot))
    }

    #[must_use]
    pub fn hotbar_slot(column: u16) -> Option<GuiPoint> {
        HOTBAR.corner(column)
    }

    #[must_use]
    pub fn inventory_entry(menu_index: u16) -> Option<GuiPoint> {
        match menu_index {
            HEAD_MENU_INDEX => Some(GuiPoint::new(54, 6)),
            CHEST_MENU_INDEX => Some(GuiPoint::new(54, 33)),
            LEGS_MENU_INDEX => Some(GuiPoint::new(108, 6)),
            FEET_MENU_INDEX => Some(GuiPoint::new(108, 33)),
            OFF_HAND_MENU_INDEX => Some(GuiPoint::new(35, 20)),
            HOTBAR_MENU_START.. => HOTBAR.corner(menu_index - HOTBAR_MENU_START),
            MAIN_MENU_START.. => MAIN_INVENTORY.corner(menu_index - MAIN_MENU_START),
            _ => None,
        }
    }

    #[must_use]
    pub const fn can_scroll(kind: CreativeTabKind, content_len: usize) -> bool {
        kind.has_scroll_bar() && content_len > GRID_SLOTS as usize
    }

    #[must_use]
    pub const fn row_count(content_len: usize) -> i32 {
        let rows = content_len.div_ceil(GRID_COLUMNS as usize);
        let rows = if rows > i32::MAX as usize {
            i32::MAX
        } else {
            rows as i32
        };
        rows.saturating_sub(GRID_ROWS as i32)
    }

    #[must_use]
    pub fn first_row(scroll: f64, row_count: i32) -> i32 {
        ((scroll * f64::from(row_count) + 0.5).floor() as i32).max(0)
    }

    #[must_use]
    pub fn thumb(scroll: f64) -> GuiRect {
        let offset = (SCROLL_TRAVEL * scroll).floor() as i32;
        GuiRect::new(
            Self::SCROLL_BAR.x,
            Self::SCROLL_BAR.y.saturating_add(offset),
            12,
            15,
        )
    }

    #[must_use]
    pub const fn thumb_sprite(
        kind: CreativeTabKind,
        content_len: usize,
    ) -> Option<ResourceLocation<'static>> {
        if !kind.has_scroll_bar() {
            None
        } else if Self::can_scroll(kind, content_len) {
            Some(Self::SCROLLER)
        } else {
            Some(Self::SCROLLER_DISABLED)
        }
    }

    #[must_use]
    pub fn scroll_from_drag(top: i32, mouse_y: f64) -> f64 {
        ((mouse_y - f64::from(top) - DRAG_OFFSET) / DRAG_TRAVEL).clamp(0.0, 1.0)
    }

    #[must_use]
    pub fn hit(
        window: GuiPoint,
        pointer: GuiPoint,
        selected: CreativeTabKind,
        visible: VisibleTabs,
    ) -> CreativeHit {
        let local = local(window, pointer);
        if let Some(index) = tab_at(local, visible, CreativeTab::click_rect) {
            return CreativeHit::Tab(index);
        }
        if !GuiRect::new(0, 0, Self::WIDTH, Self::HEIGHT).contains_point(local) {
            return CreativeHit::Outside;
        }
        match selected {
            CreativeTabKind::SurvivalInventory => survival_inventory_hit(local),
            CreativeTabKind::Search if Self::SEARCH_BOX.contains_point(local) => {
                CreativeHit::SearchBox
            }
            CreativeTabKind::Items | CreativeTabKind::SavedHotbars | CreativeTabKind::Search => {
                content_hit(local)
            }
        }
    }

    #[must_use]
    pub fn tab_under(
        window: GuiPoint,
        pointer: GuiPoint,
        visible: VisibleTabs,
    ) -> Option<CreativeTabIndex> {
        tab_at(local(window, pointer), visible, CreativeTab::hover_rect)
    }
}

fn local(window: GuiPoint, pointer: GuiPoint) -> GuiPoint {
    GuiPoint::new(
        pointer.x.saturating_sub(window.x),
        pointer.y.saturating_sub(window.y),
    )
}

fn tab_at(
    local: GuiPoint,
    visible: VisibleTabs,
    area: fn(&CreativeTab) -> GuiRect,
) -> Option<CreativeTabIndex> {
    CreativeTabIndex::all()
        .find(|tab| visible.contains(*tab) && area(tab.tab()).contains_point(local))
}

fn in_entry(corner: GuiPoint, local: GuiPoint) -> bool {
    GuiRect::new(corner.x, corner.y, ENTRY_SIZE, ENTRY_SIZE).contains_point(local)
}

fn content_hit(local: GuiPoint) -> CreativeHit {
    if let Some(index) = (0..GRID_SLOTS).find(|&index| {
        GRID.corner(index)
            .is_some_and(|corner| in_entry(corner, local))
    }) {
        return CreativeHit::GridSlot(index);
    }
    if let Some(column) = (0..GRID_COLUMNS).find(|&column| {
        HOTBAR
            .corner(column)
            .is_some_and(|corner| in_entry(corner, local))
    }) {
        return CreativeHit::HotbarSlot(column);
    }
    if CreativeGeometry::SCROLL_BAR.contains_point(local) {
        return CreativeHit::ScrollBar;
    }
    CreativeHit::Empty
}

fn survival_inventory_hit(local: GuiPoint) -> CreativeHit {
    if let Some(index) = (HEAD_MENU_INDEX..=OFF_HAND_MENU_INDEX).find(|&index| {
        CreativeGeometry::inventory_entry(index).is_some_and(|corner| in_entry(corner, local))
    }) {
        return CreativeHit::InventoryEntry(index);
    }
    if in_entry(CreativeGeometry::DESTROY_SLOT, local) {
        return CreativeHit::DestroySlot;
    }
    CreativeHit::Empty
}
