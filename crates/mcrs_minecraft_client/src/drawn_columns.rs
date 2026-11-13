use bevy::platform::collections::HashMap;
use bevy::prelude::*;
use mcrs_voxel::ColumnPos;

/// A column the client holds started or stopped being drawn. It starts once each of its sections
/// that holds blocks has a mesh placed for the frame, and stops when the client lets it go or a
/// mesh of its sections leaves the frame with none in its place.
#[derive(Message, Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DrawnColumn {
    Drawn(ColumnPos),
    Hidden(ColumnPos),
}

/// The columns the client holds, each with its sections that hold blocks against those with a
/// placed mesh, and the columns that started or stopped being drawn since the last drain.
#[derive(Default)]
pub(crate) struct DrawnColumns {
    columns: HashMap<ColumnPos, Coverage>,
    changes: Vec<DrawnColumn>,
}

#[derive(Clone, Copy, Default)]
struct Coverage {
    stored: u32,
    placed: u32,
    /// A section filled later leaves the column drawn while its first mesh is on the way.
    drawn: bool,
}

impl Coverage {
    fn drawn(self) -> bool {
        self.drawn
    }
}

impl DrawnColumns {
    pub(crate) fn arrive(&mut self, pos: ColumnPos, stored: u32) {
        self.change(pos, |held| {
            Some(Coverage {
                stored,
                placed: held.map_or(0, |held| held.placed),
                drawn: false,
            })
        });
    }

    pub(crate) fn depart(&mut self, pos: ColumnPos) {
        self.change(pos, |_| None);
    }

    /// A section of the column that held no blocks now holds one.
    pub(crate) fn fill(&mut self, pos: ColumnPos) {
        self.change(pos, |held| {
            held.map(|held| Coverage {
                stored: held.stored.saturating_add(1),
                ..held
            })
        });
    }

    pub(crate) fn place(&mut self, pos: ColumnPos) {
        self.change(pos, |held| {
            held.map(|held| Coverage {
                placed: held.placed.saturating_add(1),
                ..held
            })
        });
    }

    /// A mesh of the column left the frame with none in its place.
    pub(crate) fn unplace(&mut self, pos: ColumnPos) {
        self.change(pos, |held| {
            held.map(|held| Coverage {
                placed: held.placed.saturating_sub(1),
                drawn: false,
                ..held
            })
        });
    }

    pub(crate) fn drawn(&self) -> impl Iterator<Item = ColumnPos> {
        self.columns
            .iter()
            .filter(|(_, coverage)| coverage.drawn())
            .map(|(pos, _)| *pos)
    }

    pub(crate) fn drain(&mut self) -> impl Iterator<Item = DrawnColumn> {
        self.changes.drain(..)
    }

    fn change(&mut self, pos: ColumnPos, edit: impl FnOnce(Option<Coverage>) -> Option<Coverage>) {
        let before = self.columns.get(&pos).copied();
        let after = edit(before).map(|coverage| Coverage {
            drawn: coverage.drawn || coverage.placed >= coverage.stored,
            ..coverage
        });
        match after {
            Some(coverage) => {
                self.columns.insert(pos, coverage);
            }
            None => {
                self.columns.remove(&pos);
            }
        }
        let was = before.is_some_and(Coverage::drawn);
        let is = after.is_some_and(Coverage::drawn);
        if was != is {
            self.changes.push(if is {
                DrawnColumn::Drawn(pos)
            } else {
                DrawnColumn::Hidden(pos)
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_column_is_drawn_while_each_section_with_blocks_has_a_mesh_in_the_frame() {
        let pos = ColumnPos::new(3, -2);
        let mut columns = DrawnColumns::default();
        let mut step = |edit: fn(&mut DrawnColumns, ColumnPos)| {
            edit(&mut columns, pos);
            columns.drain().collect::<Vec<_>>()
        };
        let none: Vec<DrawnColumn> = Vec::new();
        let drawn = [DrawnColumn::Drawn(pos)];
        let hidden = [DrawnColumn::Hidden(pos)];
        assert_eq!(step(|columns, pos| columns.arrive(pos, 2)), none);
        assert_eq!(step(DrawnColumns::place), none, "one of two sections");
        assert_eq!(step(DrawnColumns::place), drawn);
        assert_eq!(
            step(DrawnColumns::fill),
            none,
            "a filled section is meshed under it"
        );
        assert_eq!(step(DrawnColumns::place), none, "the filled section's mesh");
        assert_eq!(
            step(DrawnColumns::unplace),
            hidden,
            "an evicted mesh hides it"
        );
        assert_eq!(
            step(DrawnColumns::place),
            drawn,
            "the evicted mesh placed again"
        );
        assert_eq!(step(DrawnColumns::depart), hidden, "leaving hides it");
        assert_eq!(
            step(|columns, pos| columns.arrive(pos, 1)),
            none,
            "sent again, it starts over"
        );
        assert_eq!(
            step(DrawnColumns::depart),
            none,
            "a column never drawn leaves quietly"
        );
        assert_eq!(
            step(|columns, pos| columns.arrive(pos, 0)),
            drawn,
            "a column with nothing to draw is drawn at once"
        );
        assert_eq!(step(DrawnColumns::depart), hidden);
        assert_eq!(
            step(DrawnColumns::place),
            none,
            "a column nobody holds is never drawn"
        );
    }
}
