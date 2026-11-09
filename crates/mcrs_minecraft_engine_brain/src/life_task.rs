use mcrs_minecraft_engine_physics::Mobs;
use mcrs_minecraft_entity::EntityTypeData;
use mcrs_minecraft_entity_key::EntityId;
use mcrs_minecraft_entity_key::EntityKey;
use mcrs_minecraft_entity_key::TileKey;
use mcrs_minecraft_physics::TileRows;

use crate::host_mob::HostMob;
use crate::row_hosts::row_hosts;

/// The life cycle of the mobs of one tile in one AI tick: marks each mob of the stage that is
/// still in the tile, and finds the rows that came into the tile and the mobs that left it.
pub(crate) struct LifeTask<'a, B> {
    pub(crate) tile: TileKey,
    pub(crate) rows: &'a TileRows,
    pub(crate) mobs: Option<&'a mut Vec<HostMob>>,
    pub(crate) brain_for: fn(&'static EntityTypeData) -> Option<B>,
}

/// A row of a mob with AI that the stage does not keep in its tile, with the brain of a new mob.
#[derive(Clone, Debug)]
pub(crate) struct Arrival<B> {
    pub(crate) id: EntityId,
    pub(crate) tile: TileKey,
    pub(crate) key: EntityKey,
    pub(crate) phase: u8,
    pub(crate) kind: &'static EntityTypeData,
    pub(crate) brain: B,
}

pub(crate) struct LifeOut<B> {
    pub(crate) arrivals: Vec<Arrival<B>>,
    /// The mobs of the stage that are no longer in the tile.
    pub(crate) departed: Vec<HostMob>,
}

impl<B> LifeTask<'_, B> {
    pub(crate) fn run(&mut self, tick: u64, mobs: &Mobs) -> LifeOut<B> {
        let mut out = LifeOut {
            arrivals: Vec::new(),
            departed: Vec::new(),
        };
        let slots = row_hosts(
            self.rows.ids(),
            self.mobs.as_deref().map_or(&[], Vec::as_slice),
        );
        for (row, id) in self.rows.ids().iter().enumerate() {
            if let Some(held) = self.mobs.as_deref_mut()
                && let Some(Some(at)) = slots.get(row)
                && let Some(host) = held.get_mut(*at)
            {
                host.seen = tick;
                continue;
            }
            let Some(state) = self.rows.states().get(row) else {
                continue;
            };
            if state.living.is_none() {
                continue;
            }
            let Some(mob) = mobs.0.get(id) else {
                continue;
            };
            if mob.looks.no_ai {
                continue;
            }
            if let Some(brain) = (self.brain_for)(mob.kind) {
                out.arrivals.push(Arrival {
                    id: *id,
                    tile: self.tile,
                    key: state.key,
                    phase: state.phase,
                    kind: mob.kind,
                    brain,
                });
            }
        }
        if let Some(held) = self.mobs.as_deref_mut() {
            held.retain(|host| {
                let stays = host.seen == tick;
                if !stays {
                    out.departed.push(*host);
                }
                stays
            });
        }
        out
    }
}
