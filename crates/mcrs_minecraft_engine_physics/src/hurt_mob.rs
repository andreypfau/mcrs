use bevy_ecs::entity::Entity;
use mcrs_minecraft_damage::DamageTypeRules;
use mcrs_minecraft_damage::Defence;
use mcrs_minecraft_damage::Hit;
use mcrs_minecraft_damage::HitOutcome;
use mcrs_minecraft_damage::HitRecord;
use mcrs_minecraft_damage::HitReport;
use mcrs_minecraft_damage::HurtRules;
use mcrs_minecraft_damage::Vitals;
use mcrs_minecraft_entity_key::TileKey;
use mcrs_minecraft_physics::EntityState;

use crate::mob_changes::MobChanges;
use crate::mob_changes::Struck;

pub fn hurt_mob(
    entity: Entity,
    state: &EntityState,
    vitals: &mut Vitals,
    record: &mut HitRecord,
    changes: &mut MobChanges,
    hit: Hit,
    tick: u64,
) -> HitReport {
    let report = mcrs_minecraft_damage::hurt(
        vitals,
        record,
        &Defence::default(),
        &DamageTypeRules::default(),
        &HurtRules::default(),
        hit,
        tick,
    );
    if report.outcome == HitOutcome::Rejected {
        return report;
    }
    changes.strikes.push(Struck {
        entity,
        uuid: state.key,
        tile: TileKey::at(state.body.position),
        damage_type: hit.source.damage_type,
        outcome: report.outcome,
        died: report.died,
    });
    report
}
