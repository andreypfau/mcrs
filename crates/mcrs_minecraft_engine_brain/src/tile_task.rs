use mcrs_minecraft_brain::Decision;
use mcrs_minecraft_brain::Fact;
use mcrs_minecraft_brain::FactKind;
use mcrs_minecraft_brain::MobView;
use mcrs_minecraft_brain::PanicCauses;
use mcrs_minecraft_brain::PlayerTable;
use mcrs_minecraft_entity_key::EntityId;
use mcrs_minecraft_entity_key::TileKey;
use mcrs_minecraft_entity_rows::MobIntent;
use mcrs_minecraft_entity_rows::MobRows;
use mcrs_minecraft_volume::BlockVolume;

use crate::BlockGoalWorld;
use crate::BrainState;
use crate::HotTick;
use crate::brain_state_ext::BrainStateExt;

pub(crate) struct TileTask<'a> {
    pub(crate) tile: TileKey,
    pub(crate) mobs: MobRows<'a, BrainState>,
    pub(crate) due: &'a [Decision],
    pub(crate) players: &'a PlayerTable,
    pub(crate) panic_causes: &'a PanicCauses,
    pub(crate) tick: u64,
}

pub(crate) struct TileOut {
    pub(crate) tile: TileKey,
    pub(crate) views: Vec<MobView>,
    pub(crate) facts: Vec<Fact>,
    pub(crate) ended: Vec<EntityId>,
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
        for row in self.mobs.iter_mut() {
            let observed = row.mob.observe(
                self.panic_causes,
                self.players,
                self.tick,
                row.id,
                row.state,
            );
            let kinds = [
                (observed.struck, FactKind::Hurt),
                (observed.idle_reset, FactKind::IdleReset),
                (observed.resumed, FactKind::Resumed),
            ];
            for (set, kind) in kinds {
                if set {
                    out.facts.push(Fact { mob: row.id, kind });
                }
            }
            out.views.push(observed.view);
        }
        for decision in self.due {
            if let Some(row) = self.mobs.get_mut(decision.mob) {
                row.mob.apply(&decision.intents, row.state);
                out.applied = out.applied.saturating_add(1);
            }
        }
        for row in self.mobs.iter_mut() {
            if let Some((intent, ended)) = row.mob.steer(&world, self.tick, row.state) {
                out.intents.push(MobIntent {
                    target: row.id,
                    steering: intent.steering,
                    yaw: intent.yaw,
                    pitch: intent.pitch,
                    head_yaw: intent.head_yaw,
                    body_yaw: intent.body_yaw,
                });
                if ended.is_some() {
                    out.ended.push(row.id);
                }
            }
        }
        out.views.sort_by_key(|view| view.key);
        out
    }
}
