use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use mcrs_minecraft_container_click::{Click, Clicked, StackPresence};
use mcrs_minecraft_inventory_client::{CreativeArea, CreativeClick, MenuAction};
use mcrs_minecraft_menu_geometry::CreativeHit;

use crate::creative_grid::CreativeGrid;
use crate::creative_screen_state::CreativeScreenState;
use crate::creative_tab_content::CreativeTabContent;

const DESTROY_SLOT_ENTRY: u16 = 46;

#[derive(SystemParam)]
pub(crate) struct CreativeClicks<'w> {
    screen: Option<Res<'w, CreativeScreenState>>,
    content: Option<Res<'w, CreativeTabContent>>,
}

impl CreativeClicks<'_> {
    pub(crate) fn grid(&self) -> Option<CreativeGrid<'_>> {
        Some(CreativeGrid {
            state: self.screen.as_deref()?,
            content: self.content.as_deref()?,
        })
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
