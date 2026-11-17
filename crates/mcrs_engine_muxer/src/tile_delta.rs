use crate::entity_event::SubjectEvent;
use mcrs_minecraft_entity_key::TileKey;

#[derive(Clone, Debug, PartialEq)]
pub struct TileDelta<D> {
    pub tile: TileKey,
    pub events: Vec<SubjectEvent<D>>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TileDeltas<D> {
    pub tick: u64,
    pub tiles: Vec<TileDelta<D>>,
}

impl<D> TileDeltas<D> {
    pub fn coalesce(&mut self) {
        self.tiles.sort_by_key(|delta| delta.tile);
        self.tiles.dedup_by(|later, kept| {
            if later.tile != kept.tile {
                return false;
            }
            kept.events.append(&mut later.events);
            true
        });
        for delta in &mut self.tiles {
            delta.events.sort_by_key(|event| event.subject);
        }
    }
}

impl<D> Default for TileDeltas<D> {
    fn default() -> Self {
        Self {
            tick: 0,
            tiles: Vec::new(),
        }
    }
}
