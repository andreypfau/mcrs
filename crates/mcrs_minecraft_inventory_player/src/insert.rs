use crate::HOTBAR;
use crate::MAIN;
use crate::OFF_HAND;
use crate::Stacking;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Slot {
    Empty,
    Alike { room: u8 },
    Other,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Insertion {
    pub empty: Option<u16>,
    pub filled: u8,
    pub rest: u8,
}

pub fn insert(
    count: u8,
    max: u8,
    stacking: Stacking,
    selected: u8,
    mut slot: impl FnMut(u16) -> Slot,
    mut merge: impl FnMut(u16, u8),
) -> Insertion {
    let mut rest = count;
    if stacking == Stacking::Stacks {
        let selected = Some(u16::from(selected)).filter(|index| HOTBAR.contains(index));
        let alike = selected
            .into_iter()
            .chain([OFF_HAND])
            .chain(MAIN.filter(|index| Some(*index) != selected));
        for index in alike {
            if rest == 0 {
                break;
            }
            if let Slot::Alike { room } = slot(index)
                && room > 0
            {
                let moved = rest.min(room);
                merge(index, moved);
                rest -= moved;
            }
        }
    }
    let empty = (rest > 0)
        .then(|| MAIN.clone().find(|index| slot(*index) == Slot::Empty))
        .flatten();
    let filled = if empty.is_some() { rest.min(max) } else { 0 };
    Insertion {
        empty,
        filled,
        rest: rest - filled,
    }
}
