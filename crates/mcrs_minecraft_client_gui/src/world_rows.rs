use bevy::math::IVec2;
use bevy::prelude::*;
use bevy::time::Real;
use mcrs_minecraft_client_widget::{WidgetKind, Widgets};
use mcrs_minecraft_client_world::world_list::WorldList;
use mcrs_minecraft_save::WorldSummary;

use crate::language::ClientLanguage;
use crate::menu::{Menu, RowContent};
use crate::menu_input::menu_clock;
use crate::scene::GuiAtlas;
use crate::select_world_screen::{DELETE, LIST, PLAY, search_query, shown_worlds};
use crate::world_entry::{DOTS_FRAME_MS, LocalOffset, loading_entry, world_entry};

const ROW_INSET: i32 = 2;

#[derive(Resource, Default)]
pub(crate) struct PreselectedWorld(pub(crate) Option<String>);

#[derive(Default)]
pub(crate) struct ShownRows {
    query: String,
    worlds: Vec<usize>,
    dots_frame: u64,
}

#[cfg_attr(
    mcrs_lint,
    expect(
        mcrs::alloc_in_tick,
        reason = "rebuilds the rows only when the world list, the language or the search text changes, or the loading dots advance; the search text lives in the menu widgets, and the dots follow the wall clock, so no event announces them"
    )
)]
#[cfg_attr(mcrs_lint, mcrs::alloc_sites(26))]
pub(crate) fn project_world_rows(
    mut menu: ResMut<Menu>,
    worlds: Res<WorldList>,
    language: Res<ClientLanguage>,
    atlas: Res<GuiAtlas>,
    (time, offset): (Res<Time<Real>>, Res<LocalOffset>),
    mut preselected: ResMut<PreselectedWorld>,
    mut shown: Local<ShownRows>,
) {
    let Some((rows, selected)) = list_rows(&menu.widgets) else {
        return;
    };
    let reading = matches!(*worlds, WorldList::Reading);
    let expected = if reading { 1 } else { shown.worlds.len() };
    let dots_frame = menu_clock(&time) / DOTS_FRAME_MS;
    let stale = worlds.is_changed()
        || language.is_changed()
        || rows != expected
        || search_query(&menu.widgets) != shown.query;
    if stale {
        let previous = selected
            .filter(|_| !worlds.is_changed())
            .and_then(|row| shown.worlds.get(row).copied());
        let shown = &mut *shown;
        shown.query.clear();
        shown.query.push_str(search_query(&menu.widgets));
        shown.worlds.clear();
        shown
            .worlds
            .extend(shown_worlds(&worlds, &shown.query).map(|(index, _)| index));
        let menu = &mut *menu;
        menu.rows.clear();
        if reading {
            menu.widgets.set_rows(LIST, 1);
            shown.dots_frame = dots_frame;
            menu.rows
                .push(loading(&menu.widgets, &language, &atlas, &time));
        } else {
            menu.widgets.set_rows(LIST, shown.worlds.len());
            let entries = shown.worlds.iter().enumerate();
            let list = ready(&worlds);
            for (row, world) in entries.filter_map(|(row, &index)| Some((row, list.get(index)?))) {
                let entry = world_entry(LIST, row, world, &language, &atlas.0.font, offset.0);
                menu.rows.push(entry);
            }
            let mut select =
                previous.and_then(|index| shown.worlds.iter().position(|&at| at == index));
            if let WorldList::Ready(list) = &*worlds
                && let Some(folder) = preselected.0.take()
                && select.is_none()
            {
                select = list
                    .iter()
                    .position(|world| world.folder_id == folder)
                    .and_then(|index| shown.worlds.iter().position(|&at| at == index));
            }
            menu.widgets.select_row(LIST, select);
        }
    } else if reading {
        if selected.is_some() {
            menu.widgets.select_row(LIST, None);
        }
        if shown.dots_frame != dots_frame {
            shown.dots_frame = dots_frame;
            let entry = loading(&menu.widgets, &language, &atlas, &time);
            let menu = &mut *menu;
            menu.rows.clear();
            menu.rows.push(entry);
        }
    }
    let world = list_rows(&menu.widgets)
        .and_then(|(_, selected)| selected)
        .and_then(|row| shown.worlds.get(row))
        .and_then(|&index| ready(&worlds).get(index));
    let play = world.is_some_and(|world| !world.is_disabled());
    for (id, active) in [(PLAY, play), (DELETE, world.is_some())] {
        if menu
            .widgets
            .get(id)
            .is_some_and(|button| button.active != active)
            && let Some(button) = menu.widgets.get_mut(id)
        {
            button.active = active;
        }
    }
}

fn list_rows(widgets: &Widgets) -> Option<(usize, Option<usize>)> {
    match widgets.get(LIST).map(|widget| widget.kind()) {
        Some(WidgetKind::SelectionList(list)) => Some((list.rows(), list.selected())),
        _ => None,
    }
}

fn ready(worlds: &WorldList) -> &[WorldSummary] {
    match worlds {
        WorldList::Ready(list) => list,
        WorldList::Reading | WorldList::Unavailable(_) => &[],
    }
}

fn loading(
    widgets: &Widgets,
    language: &ClientLanguage,
    atlas: &GuiAtlas,
    time: &Time<Real>,
) -> RowContent {
    let inner = widgets
        .get(LIST)
        .and_then(|widget| match widget.kind() {
            WidgetKind::SelectionList(list) => list.row_rect(widget.rect(), 0),
            _ => None,
        })
        .map_or(IVec2::ZERO, |row| {
            IVec2::new(row.x + ROW_INSET, row.y + ROW_INSET)
        });
    loading_entry(
        LIST,
        0,
        inner,
        widgets.screen().width,
        language,
        &atlas.0.font,
        menu_clock(time),
    )
}
