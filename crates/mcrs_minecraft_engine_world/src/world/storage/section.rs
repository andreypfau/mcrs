pub use mcrs_voxel::SectionPos;

use crate::entity::SectionEntities;
use crate::world::dimension::InDimension;
use crate::world::lifecycle::stage::SectionStage;
use crate::world::lifecycle::ticket::TicketPlugin;
use bevy_app::{App, Plugin};
use bevy_ecs::prelude::{Bundle, Component, Entity};
use mcrs_voxel::ColumnPos;
use rustc_hash::FxHashMap;

pub(crate) struct SectionPlugin;

impl Plugin for SectionPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(TicketPlugin);
    }
}

#[derive(Bundle)]
pub struct SectionBundle {
    pub dimension: InDimension,
    pub pos: SectionPos,
    pub entities: SectionEntities,
    marker: Section,
    stage: SectionStage,
}

#[derive(Component, Debug, Default)]
#[component(storage = "SparseSet")]
pub struct Section;

impl SectionBundle {
    pub fn new(dimension: InDimension, chunk_pos: SectionPos) -> Self {
        Self {
            dimension,
            pos: chunk_pos,
            entities: SectionEntities::default(),
            marker: Section,
            stage: SectionStage::Loading,
        }
    }
}

/// The section entities of a dimension, by column: a world holds about 24 times fewer columns
/// than sections, so the map of a lookup stays in the cache far more often.
#[derive(Component, Debug, Default)]
pub struct SectionIndex {
    columns: FxHashMap<ColumnPos, ColumnEntities>,
}

#[derive(Debug, Default)]
struct ColumnEntities {
    min_y: i32,
    entities: Vec<Option<Entity>>,
}

impl ColumnEntities {
    fn slot(&self, y: i32) -> Option<usize> {
        usize::try_from(y.checked_sub(self.min_y)?).ok()
    }
}

impl SectionIndex {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get(&self, pos: impl Into<SectionPos>) -> Option<Entity> {
        let pos = pos.into();
        let column = self.columns.get(&ColumnPos::from(pos))?;
        column.entities.get(column.slot(pos.y)?).copied().flatten()
    }

    pub fn insert(&mut self, pos: SectionPos, entity: Entity) {
        let column = self.columns.entry(ColumnPos::from(pos)).or_default();
        if column.entities.is_empty() {
            column.min_y = pos.y;
        } else if pos.y < column.min_y {
            let below = column.min_y.abs_diff(pos.y) as usize;
            column
                .entities
                .splice(0..0, core::iter::repeat_n(None, below));
            column.min_y = pos.y;
        }
        let slot = pos.y.abs_diff(column.min_y) as usize;
        if column.entities.len() <= slot {
            column.entities.resize(slot + 1, None);
        }
        if let Some(held) = column.entities.get_mut(slot) {
            *held = Some(entity);
        }
    }

    pub fn remove(&mut self, pos: impl Into<SectionPos>) -> Option<Entity> {
        let pos = pos.into();
        let at = ColumnPos::from(pos);
        let column = self.columns.get_mut(&at)?;
        let slot = column.slot(pos.y)?;
        let removed = column.entities.get_mut(slot)?.take()?;
        if column.entities.iter().all(Option::is_none) {
            self.columns.remove(&at);
        }
        Some(removed)
    }

    pub fn contains(&self, pos: impl Into<SectionPos>) -> bool {
        self.get(pos).is_some()
    }

    pub fn len(&self) -> usize {
        self.columns
            .values()
            .map(|column| column.entities.iter().flatten().count())
            .sum()
    }

    /// A column whose last section left is dropped, so no column is empty.
    pub fn is_empty(&self) -> bool {
        self.columns.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sections_inserted_in_any_height_order_are_found_until_removed() {
        let mut index = SectionIndex::new();
        let entity = |n| Entity::from_raw_u32(n).expect("a valid index");
        let at = |y| SectionPos::new(2, y, -5);
        let heights = [3, -4, 7, 0];
        for (n, y) in (1..).zip(heights) {
            index.insert(at(y), entity(n));
        }
        let neighbour = SectionPos::new(3, 3, -5);
        index.insert(neighbour, entity(9));

        for (n, y) in (1..).zip(heights) {
            assert_eq!(index.get(at(y)), Some(entity(n)));
        }
        assert_eq!(index.get(at(1)), None, "a gap");
        assert_eq!(index.get(at(-5)), None, "below the lowest");
        assert_eq!(index.get(at(8)), None, "above the highest");
        assert_eq!(index.len(), 5);

        index.insert(at(3), entity(8));
        assert_eq!(index.get(at(3)), Some(entity(8)), "replaced");
        assert_eq!(index.len(), 5);

        for y in heights {
            assert!(index.remove(at(y)).is_some());
        }
        assert_eq!(index.remove(at(3)), None);
        assert_eq!(index.get(neighbour), Some(entity(9)));
        assert_eq!(index.len(), 1);
    }
}
