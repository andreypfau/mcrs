use alloc::collections::BTreeMap;
use alloc::collections::VecDeque;
use alloc::vec::Vec;

use crate::Fact;
use crate::MobView;
use crate::PlayerTable;
use mcrs_minecraft_entity_key::TileKey;

/// The snapshots that the decisions of the last ticks read. A tile keeps a version for each tick
/// in which the host published it, so a decision of issue tick `T` processed late still reads the
/// views of `T`; a tile that the host did not publish again keeps its older version.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Replica {
    tiles: BTreeMap<TileKey, VecDeque<TileVersion>>,
    players: VecDeque<PlayersVersion>,
    facts: VecDeque<TickFacts>,
}

#[derive(Clone, Debug, PartialEq)]
struct TileVersion {
    tick: u64,
    views: Vec<MobView>,
}

#[derive(Clone, Debug, PartialEq)]
struct PlayersVersion {
    tick: u64,
    table: PlayerTable,
}

#[derive(Clone, Debug, Default, PartialEq)]
struct TickFacts {
    tick: u64,
    tiles: BTreeMap<TileKey, Vec<Fact>>,
}

impl Replica {
    /// Publishes the views of the mobs of `tile` at the start of `tick`. The stage expects one
    /// view for each mob that it keeps in the tile; the views are sorted here by order key.
    pub fn publish_tile(&mut self, tick: u64, tile: TileKey, mut views: Vec<MobView>) {
        views.sort_by_key(|view| view.key);
        let versions = self.tiles.entry(tile).or_default();
        if let Some(last) = versions.back_mut()
            && last.tick == tick
        {
            last.views = views;
            return;
        }
        versions.push_back(TileVersion { tick, views });
    }

    pub fn publish_players(&mut self, tick: u64, table: PlayerTable) {
        if let Some(last) = self.players.back_mut()
            && last.tick == tick
        {
            last.table = table;
            return;
        }
        self.players.push_back(PlayersVersion { tick, table });
    }

    /// Publishes the facts of the ticks before `tick` for the mobs of `tile`.
    pub fn publish_facts(
        &mut self,
        tick: u64,
        tile: TileKey,
        facts: impl IntoIterator<Item = Fact>,
    ) {
        let at = match self.facts.back() {
            Some(last) if last.tick == tick => self.facts.len() - 1,
            _ => {
                self.facts.push_back(TickFacts {
                    tick,
                    tiles: BTreeMap::new(),
                });
                self.facts.len() - 1
            }
        };
        if let Some(entry) = self.facts.get_mut(at) {
            entry.tiles.entry(tile).or_default().extend(facts);
        }
    }

    /// The views of `tile` at the start of `tick`: the newest version at `tick` or before.
    #[must_use]
    pub fn views(&self, tile: TileKey, tick: u64) -> &[MobView] {
        self.tiles
            .get(&tile)
            .and_then(|versions| versions.iter().rev().find(|version| version.tick <= tick))
            .map_or(&[], |version| version.views.as_slice())
    }

    #[must_use]
    pub fn players(&self, tick: u64) -> Option<&PlayerTable> {
        self.players
            .iter()
            .rev()
            .find(|version| version.tick <= tick)
            .map(|version| &version.table)
    }

    /// The facts that the host published with the snapshot of `tick` for `tile`.
    #[must_use]
    pub fn facts(&self, tile: TileKey, tick: u64) -> &[Fact] {
        self.facts
            .iter()
            .find(|entry| entry.tick == tick)
            .and_then(|entry| entry.tiles.get(&tile))
            .map_or(&[], Vec::as_slice)
    }

    /// Drops what no read at `oldest` or later needs: each version with a newer version at
    /// `oldest` or before, and the facts of the ticks before `oldest`.
    pub fn retire(&mut self, oldest: u64) {
        self.tiles.retain(|_, versions| {
            retire_versions(versions, oldest, |version| version.tick);
            versions
                .back()
                .is_some_and(|version| !version.views.is_empty())
                || versions.len() > 1
        });
        retire_versions(&mut self.players, oldest, |version| version.tick);
        while self.facts.front().is_some_and(|entry| entry.tick < oldest) {
            self.facts.pop_front();
        }
    }
}

fn retire_versions<T>(versions: &mut VecDeque<T>, oldest: u64, tick: impl Fn(&T) -> u64) {
    while versions.len() > 1 && versions.get(1).is_some_and(|next| tick(next) <= oldest) {
        versions.pop_front();
    }
}
