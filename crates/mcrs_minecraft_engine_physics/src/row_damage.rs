use mcrs_minecraft_damage::Hit;
use mcrs_minecraft_damage::HitOutcome;
use mcrs_minecraft_entity_rows::Placed;
use mcrs_minecraft_entity_rows::RowStore;

use crate::row_mob_life::RowMobChanges;
use crate::row_mob_life::RowStruck;

pub fn hurt_row<M>(
    rows: &mut RowStore<M>,
    changes: &mut RowMobChanges,
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
    changes.strikes.push(RowStruck {
        at,
        damage_type: hit.damage_type,
        outcome,
        died: !living.is_alive(),
    });
    changes.touched.push(at);
    Some(outcome)
}

pub(crate) fn report_crowd_hits<M>(
    rows: &RowStore<M>,
    changes: &mut RowMobChanges,
    damage_type: u32,
) {
    for hit in rows.crowd_hits() {
        changes.strikes.push(RowStruck {
            at: hit.at,
            damage_type,
            outcome: hit.outcome,
            died: hit.died,
        });
        changes.touched.push(hit.at);
    }
}
