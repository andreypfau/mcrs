use mcrs_minecraft_damage::HitOutcome;
use mcrs_minecraft_damage::compute_knockback;
use mcrs_minecraft_damage::{Defence, Hit};
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
    defence: &Defence,
    tick: u64,
) -> Option<HitOutcome> {
    let state = rows.state_mut(at)?;
    let living = state.living.as_mut()?;
    let defence = Defence {
        fire_immune: living.fire_immune,
        fall_immune: living.fall_damage_immune,
        ..*defence
    };
    let outcome =
        mcrs_minecraft_damage::hurt(&mut living.vitals, &mut state.hits, &defence, hit, tick);
    if outcome == HitOutcome::Rejected {
        return Some(outcome);
    }
    if !defence.flags.no_knockback
        && let Some(attacker) = hit.source.source_position
    {
        let target = state.body.position;
        state.body.velocity = compute_knockback(
            target,
            attacker,
            f64::from(hit.knockback),
            0.0,
            state.body.velocity,
        );
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

/// Queues what the viewers see of the crowd hits of the last pass of `rows`, which have the
/// damage type `damage_type` (DMG-CRW-001).
pub(crate) fn report_crowd_hits<M>(rows: &RowStore<M>, changes: &mut MobChanges, damage_type: u32) {
    for hit in rows.crowd_hits() {
        changes.strikes.push(Struck {
            at: hit.at,
            damage_type,
            outcome: hit.outcome,
            died: hit.died,
        });
        changes.touched.push(hit.at);
    }
}

#[cfg(test)]
mod tests {
    use bevy_ecs::resource::Resource;
    use glam::DVec3;
    use mcrs_engine_muxer::EntityId;
    use mcrs_minecraft_damage::{Defence, Hit};
    use mcrs_minecraft_entity::EntityTypeData;
    use mcrs_minecraft_entity_key::EntityKey;
    use mcrs_minecraft_entity_rows::RowStore;

    use super::*;
    use crate::mob_life::{mob_body, mob_row};

    #[derive(Resource)]
    struct Marker;

    fn spawn_row(kind: &'static EntityTypeData) -> (RowStore<Marker>, Placed) {
        let mut rows = RowStore::<Marker>::default();
        let row = mob_row(kind);
        rows.spawn(
            EntityId(2),
            mob_body(kind, &row, EntityKey(2), DVec3::new(0.5, 0.0, 0.5)),
            row,
        );
        let at = rows.living_rows().next().expect("the mob row exists").0;
        (rows, at)
    }

    #[test]
    fn dmg_hit_001_a_fire_immune_mob_rejects_fire_damage() {
        let (mut rows, at) = spawn_row(&EntityTypeData::ZOMBIFIED_PIGLIN);
        let outcome = hurt_mob(
            &mut rows,
            &mut MobChanges::default(),
            at,
            Hit::new(5.0, 0),
            &Defence::new(mcrs_minecraft_damage::DamageTypeFlags {
                is_fire: true,
                ..mcrs_minecraft_damage::DamageTypeFlags::default()
            }),
            0,
        );
        assert_eq!(outcome, Some(HitOutcome::Rejected));
        assert_eq!(
            rows.state_of(at)
                .and_then(|state| state.living)
                .map(|l| l.vitals.health),
            Some(20.0)
        );
    }

    #[test]
    fn dmg_hit_001_a_fall_immune_mob_rejects_fall_damage() {
        let (mut rows, at) = spawn_row(&EntityTypeData::CHICKEN);
        let outcome = hurt_mob(
            &mut rows,
            &mut MobChanges::default(),
            at,
            Hit::new(5.0, 0),
            &Defence::new(mcrs_minecraft_damage::DamageTypeFlags {
                is_fall: true,
                ..mcrs_minecraft_damage::DamageTypeFlags::default()
            }),
            0,
        );
        assert_eq!(outcome, Some(HitOutcome::Rejected));
        assert_eq!(
            rows.state_of(at)
                .and_then(|state| state.living)
                .map(|l| l.vitals.health),
            Some(4.0)
        );
    }
}
