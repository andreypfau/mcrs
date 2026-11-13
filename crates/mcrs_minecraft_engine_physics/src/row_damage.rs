use bevy_ecs::message::Message;
use bevy_ecs::message::MessageWriter;
use bevy_ecs::resource::Resource;
use bevy_ecs::schedule::SystemSet;
use bevy_ecs::system::ResMut;
use mcrs_minecraft_damage::DamageSource;
use mcrs_minecraft_damage::Defence;
use mcrs_minecraft_damage::Hit;
use mcrs_minecraft_damage::HitOutcome;
use mcrs_minecraft_entity_rows::Placed;
use mcrs_minecraft_entity_rows::RowStore;
use mcrs_minecraft_entity_rows::TileQueues;

use crate::mob_damage::hurt_mob;
use crate::mob_life::MobChanges;

/// A hit on a row, waiting for the merge point of its tile.
#[derive(Clone, Debug, PartialEq)]
pub struct RowHit {
    pub at: Placed,
    pub hit: Hit,
    pub defence: Defence,
    pub tick: u64,
}

/// The point of `Simulate` at which the queued row hits are applied, after the entity pass and
/// after the combat systems queued them.
#[derive(SystemSet, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct RowHitApply;

/// The hits that the combat systems queued for the rows this tick, by tile.
#[derive(Resource, Debug, Default)]
pub struct RowHits(pub TileQueues<RowHit>);

/// A row that took a hit at the merge point.
#[derive(Message, Clone, Debug, PartialEq)]
pub struct RowDamaged {
    pub at: Placed,
    pub hit: Hit,
    pub outcome: HitOutcome,
    pub damage: f32,
    pub died: bool,
}

/// A row that died at the merge point.
#[derive(Message, Clone, Debug, PartialEq)]
pub struct RowDied {
    pub at: Placed,
    pub source: DamageSource,
}

pub(crate) fn apply_row_hits<M: Send + Sync + 'static>(
    mut hits: ResMut<RowHits>,
    mut rows: ResMut<RowStore<M>>,
    mut changes: ResMut<MobChanges>,
    mut damaged: MessageWriter<RowDamaged>,
    mut died: MessageWriter<RowDied>,
) {
    for (_, entries) in hits.0.drain() {
        for row in entries {
            let before = rows
                .state_of(row.at)
                .and_then(|state| state.living)
                .map_or(0.0, |living| {
                    living.vitals.health + living.vitals.absorption
                });
            let Some(outcome) = hurt_mob(
                &mut *rows,
                &mut changes,
                row.at,
                row.hit,
                &row.defence,
                row.tick,
            ) else {
                continue;
            };
            if outcome == HitOutcome::Rejected {
                continue;
            }
            let after = rows
                .state_of(row.at)
                .and_then(|state| state.living)
                .map_or(0.0, |living| {
                    living.vitals.health + living.vitals.absorption
                });
            let died_now = rows
                .state_of(row.at)
                .and_then(|state| state.living)
                .is_some_and(|living| !living.is_alive());
            damaged.write(RowDamaged {
                at: row.at,
                hit: row.hit,
                outcome,
                damage: (before - after).max(0.0),
                died: died_now,
            });
            if died_now {
                died.write(RowDied {
                    at: row.at,
                    source: row.hit.source,
                });
            }
        }
    }
}
