use mcrs_minecraft_damage::Hit;
use mcrs_minecraft_damage::HitOutcome;
use mcrs_minecraft_entity_rows::Placed;
use mcrs_minecraft_entity_rows::RowStore;

use crate::mob_life::MobChanges;
use crate::mob_life::Struck;

/// Hurts the mob row `at` and queues what its viewers see; `None` when `at` is not a living row.
pub fn hurt_mob<M>(
    rows: &mut RowStore<M>,
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
    changes.strikes.push(Struck {
        at,
        damage_type: hit.damage_type,
        outcome,
        died: !living.is_alive(),
    });
    changes.touched.push(at);
    Some(outcome)
}
