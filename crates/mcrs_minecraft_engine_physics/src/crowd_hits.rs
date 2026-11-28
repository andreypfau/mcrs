use bevy_ecs::system::Res;
use bevy_ecs::system::ResMut;
use mcrs_minecraft_damage::DamageSource;
use mcrs_minecraft_damage::Hit;
use mcrs_minecraft_engine_damage::CrowdCount;
use mcrs_minecraft_engine_damage::DamageRequest;
use mcrs_minecraft_engine_damage::PendingHits;
use mcrs_minecraft_engine_entity_motion::CrowdCandidates;
use mcrs_minecraft_engine_entity_motion::Expired;
use mcrs_minecraft_engine_world::birth_clock::BirthClock;
use mcrs_minecraft_engine_world::game_rules::GameRules;
use mcrs_minecraft_entity_key::WriterKey;
use mcrs_minecraft_physics::CROWD_DAMAGE;

use crate::crowd_damage_type::CrowdDamageType;
use crate::entity_motion_plugin::push_rules;

pub(crate) fn has_crowds(crowds: Res<CrowdCandidates>) -> bool {
    !crowds.entries().is_empty()
}

pub(crate) fn hit_crowds(
    crowds: Res<CrowdCandidates>,
    expired: Res<Expired>,
    damage_type: Res<CrowdDamageType>,
    clock: Res<BirthClock>,
    rules: Option<Res<GameRules>>,
    mut pending: ResMut<PendingHits>,
) {
    let Some(damage_type) = damage_type.0 else {
        return;
    };
    let hit = Hit::new(CROWD_DAMAGE, DamageSource::of(damage_type));
    let limit = i64::from(push_rules(rules.as_deref()).max_entity_cramming) - 1;
    let gone = expired.entries();
    for candidate in crowds.entries() {
        let own = candidate.own;
        if gone
            .binary_search_by_key(&own.key, |expiry| expiry.key)
            .is_ok()
        {
            continue;
        }
        let writer = WriterKey::Entity { key: own.key };
        let crowd = CrowdCount {
            own,
            candidates: candidate.candidates,
            free: candidate.free,
            limit,
        };
        pending.push(DamageRequest::new(candidate.entity, hit, writer, clock.0, 0).in_crowd(crowd));
    }
}
