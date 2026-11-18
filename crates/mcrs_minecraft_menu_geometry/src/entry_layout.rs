use mcrs_minecraft_client_widget::GuiPoint;

use crate::grid::Grid;

const PLAYER_COLUMNS: u16 = 9;
const PLAYER_ROWS: u16 = 3;
const PLAYER_ENTRIES: u16 = PLAYER_COLUMNS * PLAYER_ROWS;
const HOTBAR_OFFSET: i32 = 58;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum EntryLayout {
    Table(&'static [GuiPoint]),
    Generic { container: Grid, player: GuiPoint },
}

impl EntryLayout {
    pub(crate) fn len(self) -> u16 {
        match self {
            Self::Table(entries) => u16::try_from(entries.len()).unwrap_or(u16::MAX),
            Self::Generic { container, .. } => container
                .len()
                .saturating_add(PLAYER_ENTRIES)
                .saturating_add(PLAYER_COLUMNS),
        }
    }

    pub(crate) fn corner(self, index: u16) -> Option<GuiPoint> {
        match self {
            Self::Table(entries) => entries.get(usize::from(index)).copied(),
            Self::Generic { container, player } => {
                let Some(player_index) = index.checked_sub(container.len()) else {
                    return container.corner(index);
                };
                match player_index.checked_sub(PLAYER_ENTRIES) {
                    None => Grid {
                        origin: player,
                        columns: PLAYER_COLUMNS,
                        rows: PLAYER_ROWS,
                    }
                    .corner(player_index),
                    Some(hotbar_index) => Grid {
                        origin: GuiPoint::new(player.x, player.y + HOTBAR_OFFSET),
                        columns: PLAYER_COLUMNS,
                        rows: 1,
                    }
                    .corner(hotbar_index),
                }
            }
        }
    }
}
