use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use mcrs_minecraft_container_click::{Click, Clicked, StackPresence};
use mcrs_minecraft_container_gesture::GestureHit;
use mcrs_minecraft_inventory_client::{CreativeArea, CreativeClick, MenuAction};
use mcrs_minecraft_menu_geometry::CreativeHit;

use crate::creative_grid::CreativeGrid;
use crate::creative_screen_state::CreativeScreenState;
use crate::creative_tab_content::CreativeTabContent;
use crate::creative_target::CreativeTarget;

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

pub(crate) fn hit(hit: CreativeHit, grid: Option<CreativeGrid>) -> GestureHit {
    GestureHit {
        clicked: target_of(hit, grid).map(|target| target.clicked),
        entry: hit.menu_index(),
        target: target(hit, grid),
        takes_left_press: matches!(hit, CreativeHit::Tab(_) | CreativeHit::ScrollBar),
        permits_drag: !matches!(hit, CreativeHit::GridSlot(_) | CreativeHit::DestroySlot),
    }
}

pub(crate) fn target_of(hit: CreativeHit, grid: Option<CreativeGrid>) -> Option<CreativeTarget> {
    match hit {
        CreativeHit::GridSlot(slot) => Some(CreativeTarget {
            clicked: Clicked::Entry(slot),
            area: CreativeArea::Grid(grid.and_then(|grid| grid.stack(slot)).cloned()),
        }),
        CreativeHit::DestroySlot => Some(CreativeTarget {
            clicked: Clicked::Entry(DESTROY_SLOT_ENTRY),
            area: CreativeArea::Destroy,
        }),
        CreativeHit::Outside => Some(CreativeTarget {
            clicked: Clicked::Outside,
            area: CreativeArea::Inventory,
        }),
        CreativeHit::HotbarSlot(_) => Some(CreativeTarget {
            clicked: hit.menu_index().map_or(Clicked::Nowhere, Clicked::Entry),
            area: CreativeArea::Hotbar,
        }),
        CreativeHit::InventoryEntry(_) => Some(CreativeTarget {
            clicked: hit.menu_index().map_or(Clicked::Nowhere, Clicked::Entry),
            area: CreativeArea::Inventory,
        }),
        CreativeHit::Tab(_)
        | CreativeHit::ScrollBar
        | CreativeHit::SearchBox
        | CreativeHit::Empty => None,
    }
}

pub(crate) fn entry_click(click: Click) -> MenuAction {
    MenuAction::Creative(CreativeClick {
        area: CreativeArea::Inventory,
        click,
    })
}

fn target(hit: CreativeHit, grid: Option<CreativeGrid>) -> Option<StackPresence> {
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
