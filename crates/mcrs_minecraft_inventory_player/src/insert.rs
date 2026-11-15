use crate::HOTBAR;
use crate::MAIN;
use crate::OFF_HAND;
use crate::Stacking;

/// What a slot holds, seen from the stack that goes in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Slot {
    Empty,
    /// An alike stack with room for `room` more items.
    Alike {
        room: u8,
    },
    /// A stack that is not alike, or a slot that does not exist.
    Other,
}

/// The end of an insertion: the empty slot that takes `filled` items, and the items that found
/// no room.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Insertion {
    pub empty: Option<u16>,
    pub filled: u8,
    pub rest: u8,
}

/// Inserts `count` items whose maximum stack size is `max` into a player inventory whose
/// selected hotbar slot is `selected`. `merge` gets each alike slot with the items that it takes,
/// in the order of the rule. A stack that stays `Alone` goes only to the first empty main slot. The insert
/// reads no place filter and never fills an armour slot or an empty off-hand slot.
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
