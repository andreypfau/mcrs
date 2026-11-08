use mcrs_engine_muxer::EntityId;
use mcrs_engine_muxer::TileKey;
use mcrs_minecraft_engine_physics::Mobs;
use mcrs_minecraft_mob::MobType;
use mcrs_minecraft_physics::BirthKey;
use mcrs_minecraft_physics::TileRows;

use crate::host_mob::HostMob;

/// The life cycle of the mobs of one tile in one AI tick: marks each mob of the stage that is
/// still in the tile, and finds the rows that came into the tile and the mobs that left it.
pub(crate) struct LifeTask<'a> {
    pub(crate) tile: TileKey,
    pub(crate) rows: &'a TileRows,
    pub(crate) mobs: Option<&'a mut Vec<HostMob>>,
}

/// A row of a pig with AI that the stage does not keep in its tile.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Arrival {
    pub(crate) id: EntityId,
    pub(crate) tile: TileKey,
    pub(crate) key: BirthKey,
    pub(crate) phase: u8,
    pub(crate) kind: MobType,
}

#[derive(Default)]
pub(crate) struct LifeOut {
    pub(crate) arrivals: Vec<Arrival>,
    /// The mobs of the stage that are no longer in the tile.
    pub(crate) departed: Vec<HostMob>,
}

impl LifeTask<'_> {
    pub(crate) fn run(&mut self, tick: u64, mobs: &Mobs) -> LifeOut {
        let mut out = LifeOut::default();
        for (id, state) in self.rows.ids().iter().zip(self.rows.states()) {
            if state.living.is_none() {
                continue;
            }
            if let Some(held) = self.mobs.as_deref_mut()
                && let Ok(at) = held.binary_search_by_key(id, |host| host.id)
                && let Some(host) = held.get_mut(at)
            {
                host.seen = tick;
                continue;
            }
            let Some(mob) = mobs.0.get(id) else {
                continue;
            };
            if mob.kind == MobType::Pig && !mob.looks.no_ai {
                out.arrivals.push(Arrival {
                    id: *id,
                    tile: self.tile,
                    key: state.key,
                    phase: state.phase,
                    kind: mob.kind,
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
