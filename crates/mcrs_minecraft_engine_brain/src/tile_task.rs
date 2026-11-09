use mcrs_engine_muxer::TileKey;
use mcrs_minecraft_brain::Decision;
use mcrs_minecraft_brain::Fact;
use mcrs_minecraft_brain::FactKind;
use mcrs_minecraft_brain::MobId;
use mcrs_minecraft_brain::MobView;
use mcrs_minecraft_engine_physics::MobIntent;
use mcrs_minecraft_physics::TileRows;
use mcrs_minecraft_volume::BlockVolume;

use crate::BlockGoalWorld;
use crate::HotTick;
use crate::host_mob::HostMob;
use crate::row_hosts::row_hosts;

/// The hot side of one tile in one AI tick: the snapshot of its mobs, the application of the
/// due decisions in the order of their issue ticks and keys, and the controls before the travel.
pub(crate) struct TileTask<'a> {
    pub(crate) tile: TileKey,
    pub(crate) rows: &'a TileRows,
    pub(crate) mobs: &'a mut [HostMob],
    pub(crate) due: &'a [Decision],
    pub(crate) tick: u64,
}

/// What the task of one tile gives back.
pub(crate) struct TileOut {
    pub(crate) tile: TileKey,
    pub(crate) views: Vec<MobView>,
    /// The wake events for the snapshot of this tick.
    pub(crate) facts: Vec<Fact>,
    /// The mobs whose path ended in this tick, for the snapshot of the next one.
    pub(crate) ended: Vec<MobId>,
    pub(crate) intents: Vec<MobIntent>,
    pub(crate) applied: usize,
}

impl TileTask<'_> {
    pub(crate) fn run<V: BlockVolume>(&mut self, hot: &HotTick<'_, V>) -> TileOut {
        let world = BlockGoalWorld::new(hot.blocks, hot.collision);
        let mut out = TileOut {
            tile: self.tile,
            views: Vec::with_capacity(self.mobs.len()),
            facts: Vec::new(),
            ended: Vec::new(),
            intents: Vec::with_capacity(self.mobs.len()),
            applied: 0,
        };
        let rows = self.rows;
        let slots = row_hosts(rows.ids(), self.mobs);
        for (slot, state) in slots.iter().zip(rows.states()) {
            let Some(mob) = slot.and_then(|at| self.mobs.get_mut(at)) else {
                continue;
            };
            let observed = mob.observe(hot, self.tick, state);
            let kinds = [
                (observed.struck, FactKind::Hurt),
                (observed.idle_reset, FactKind::IdleReset),
            ];
            for (set, kind) in kinds {
                if set {
                    out.facts.push(Fact {
                        mob: mob.mob(),
                        kind,
                    });
                }
            }
            out.views.push(observed.view);
        }
        for decision in self.due {
            if let Some(mob) = find(self.mobs, decision.mob.0) {
                mob.apply(&decision.intents);
                out.applied = out.applied.saturating_add(1);
            }
        }
        for mob in self.mobs.iter_mut() {
            if let Some((intent, ended)) = mob.steer(&world, self.tick) {
                out.intents.push(intent);
                if ended.is_some() {
                    out.ended.push(mob.mob());
                }
            }
        }
        out.views.sort_by_key(|view| view.key);
        out
    }
}

fn find(mobs: &mut [HostMob], id: u64) -> Option<&mut HostMob> {
    let at = mobs.binary_search_by_key(&id, |mob| mob.id.0).ok()?;
    mobs.get_mut(at)
}
