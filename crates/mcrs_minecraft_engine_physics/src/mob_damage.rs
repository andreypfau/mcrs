use mcrs_minecraft_damage::Hit;
use mcrs_minecraft_damage::HitOutcome;
use mcrs_minecraft_entity_rows::Placed;
use mcrs_minecraft_replication::MobSignal;

use crate::EntityRows;
use crate::mob_life::MobChanges;
use crate::mob_life::Signaled;

/// Hurts the mob row `at` and queues what its viewers see; `None` when `at` is not a living row.
pub fn hurt_mob(
    rows: &mut EntityRows,
    changes: &mut MobChanges,
    at: Placed,
    hit: Hit,
    tick: u64,
) -> Option<HitOutcome> {
    let state = rows.state_mut(at)?;
    let living = state.living.as_mut()?;
    let outcome = mcrs_minecraft_damage::hurt(living, &mut state.hits, hit, tick);
    if outcome == HitOutcome::Rejected {
        return Some(outcome);
    }
    let mut signal = |signal| changes.signals.push(Signaled { at, signal });
    if outcome == HitOutcome::Full {
        signal(MobSignal::Hurt {
            damage_type: hit.damage_type,
        });
    }
    if !living.is_alive() {
        signal(MobSignal::Died);
    }
    changes.touched.push(at);
    Some(outcome)
}
