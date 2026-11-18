use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use mcrs_minecraft_client_widget::GuiPoint;
use mcrs_minecraft_container_click::{Click, Clicked, StackPresence};
use mcrs_minecraft_inventory_client::{CreativeArea, CreativeClick, MenuAction};
use mcrs_minecraft_item_stack::ItemStack;
use mcrs_minecraft_menu_geometry::{
    CreativeGeometry, CreativeHit, CreativeTabIndex, CreativeTabKind, VisibleTabs,
};

use crate::creative_screen_state::CreativeScreenState;
use crate::creative_tab_content::CreativeTabContent;

const DESTROY_SLOT_ENTRY: u16 = 46;

#[derive(SystemParam)]
pub(crate) struct CreativeClicks<'w> {
    screen: Option<Res<'w, CreativeScreenState>>,
    content: Option<Res<'w, CreativeTabContent>>,
}

#[derive(SystemParam)]
pub(crate) struct CreativeTabSelect<'w> {
    screen: Option<ResMut<'w, CreativeScreenState>>,
    content: Option<Res<'w, CreativeTabContent>>,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct CreativeLayout {
    selected: CreativeTabKind,
    visible: VisibleTabs,
}

#[derive(Clone, Copy)]
pub(crate) struct CreativeGrid<'a> {
    state: &'a CreativeScreenState,
    content: &'a CreativeTabContent,
}

impl CreativeClicks<'_> {
    pub(crate) fn layout(&self) -> Option<CreativeLayout> {
        let (screen, content) = (self.screen.as_deref()?, self.content.as_deref()?);
        Some(CreativeLayout {
            selected: screen.selected_tab().kind,
            visible: content.visible_tabs(),
        })
    }

    pub(crate) fn grid(&self) -> Option<CreativeGrid<'_>> {
        Some(CreativeGrid {
            state: self.screen.as_deref()?,
            content: self.content.as_deref()?,
        })
    }
}

impl CreativeTabSelect<'_> {
    pub(crate) fn select(&mut self, tab: CreativeTabIndex) {
        if let (Some(screen), Some(content)) = (self.screen.as_mut(), self.content.as_deref()) {
            screen.select(tab, content);
        }
    }
}

impl CreativeLayout {
    pub(crate) fn hit(self, width: i32, height: i32, pointer: GuiPoint) -> CreativeHit {
        let window = CreativeGeometry::window(width, height);
        CreativeGeometry::hit(window, pointer, self.selected, self.visible)
    }

    pub(crate) fn inventory_key_closes(self) -> bool {
        self.selected != CreativeTabKind::Search
    }
}

impl<'a> CreativeGrid<'a> {
    fn stack(self, slot: u16) -> Option<&'a ItemStack> {
        self.state.grid_stack(slot, self.content)
    }
}

pub(crate) fn clicked(hit: CreativeHit) -> Clicked {
    match hit {
        CreativeHit::GridSlot(slot) => Clicked::Entry(slot),
        CreativeHit::DestroySlot => Clicked::Entry(DESTROY_SLOT_ENTRY),
        CreativeHit::Outside => Clicked::Outside,
        CreativeHit::HotbarSlot(_)
        | CreativeHit::InventoryEntry(_)
        | CreativeHit::Tab(_)
        | CreativeHit::ScrollBar
        | CreativeHit::SearchBox
        | CreativeHit::Empty => hit.menu_index().map_or(Clicked::Nowhere, Clicked::Entry),
    }
}

pub(crate) fn entry_click(click: Click) -> MenuAction {
    MenuAction::Creative(CreativeClick {
        area: CreativeArea::Inventory,
        click,
    })
}

pub(crate) fn permits_drag(hit: CreativeHit) -> bool {
    !matches!(hit, CreativeHit::GridSlot(_) | CreativeHit::DestroySlot)
}

pub(crate) fn target(hit: CreativeHit, grid: Option<CreativeGrid>) -> Option<StackPresence> {
    match hit {
        CreativeHit::GridSlot(slot) => Some(match grid.and_then(|grid| grid.stack(slot)) {
            Some(_) => StackPresence::Stack,
            None => StackPresence::Empty,
        }),
        CreativeHit::DestroySlot => Some(StackPresence::Empty),
        CreativeHit::HotbarSlot(_)
        | CreativeHit::InventoryEntry(_)
        | CreativeHit::Tab(_)
        | CreativeHit::ScrollBar
        | CreativeHit::SearchBox
        | CreativeHit::Empty
        | CreativeHit::Outside => None,
    }
}

pub(crate) fn area(hit: CreativeHit, grid: Option<CreativeGrid>) -> Option<CreativeArea> {
    match hit {
        CreativeHit::GridSlot(slot) => Some(CreativeArea::Grid(
            grid.and_then(|grid| grid.stack(slot)).cloned(),
        )),
        CreativeHit::HotbarSlot(_) => Some(CreativeArea::Hotbar),
        CreativeHit::InventoryEntry(_) | CreativeHit::Outside => Some(CreativeArea::Inventory),
        CreativeHit::DestroySlot => Some(CreativeArea::Destroy),
        CreativeHit::Tab(_)
        | CreativeHit::ScrollBar
        | CreativeHit::SearchBox
        | CreativeHit::Empty => None,
    }
}
