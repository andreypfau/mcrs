use bevy_ecs::system::Local;
use bevy_ecs::system::Query;
use bevy_ecs::system::Res;
use bevy_ecs::system::ResMut;
use mcrs_minecraft_damage::Hit;
use mcrs_minecraft_damage::HitOutcome;
use mcrs_minecraft_engine_entity_motion::CrowdCandidate;
use mcrs_minecraft_engine_entity_motion::CrowdCandidates;
use mcrs_minecraft_engine_entity_motion::Expired;
use mcrs_minecraft_engine_entity_motion::MotionState;
use mcrs_minecraft_engine_world::birth_clock::BirthClock;
use mcrs_minecraft_engine_world::game_rules::GameRules;
use mcrs_minecraft_physics::CROWD_DAMAGE;
use mcrs_minecraft_physics::GridFlags;
use mcrs_minecraft_physics::GridRow;
use mcrs_minecraft_physics::Rest;
use mcrs_minecraft_physics::pushes;

use crate::entity_motion_plugin::push_rules;
use crate::hurt_mob::hurt_mob;
use crate::mob_changes::MobChanges;
use crate::row_physics_plugin::CrowdDamageType;

pub(crate) fn has_crowds(crowds: Res<CrowdCandidates>) -> bool {
    !crowds.entries().is_empty()
}

#[expect(
    clippy::too_many_arguments,
    reason = "a Bevy system: each resource and query is its own parameter so the scheduler sees the access"
)]
pub(crate) fn hit_crowds(
    crowds: Res<CrowdCandidates>,
    expired: Res<Expired>,
    damage_type: Res<CrowdDamageType>,
    clock: Res<BirthClock>,
    rules: Option<Res<GameRules>>,
    mut states: Query<&mut MotionState>,
    mut changes: ResMut<MobChanges>,
    mut dead: Local<Vec<GridRow>>,
) {
    let Some(damage_type) = damage_type.0 else {
        return;
    };
    let hit = Hit::new(CROWD_DAMAGE, damage_type);
    let limit = i64::from(push_rules(rules.as_deref()).max_entity_cramming) - 1;
    dead.clear();
    for candidate in crowds.entries() {
        if !overcrowded(candidate, &dead, limit) {
            continue;
        }
        let own = candidate.own;
        let gone = expired.entries();
        if gone
            .binary_search_by_key(&own.key, |expiry| expiry.key)
            .is_ok()
        {
            continue;
        }
        let Ok(mut state) = states.get_mut(candidate.entity) else {
            continue;
        };
        let state = &mut state.0;
        let outcome = hurt_mob(candidate.entity, state, &mut changes, hit, clock.0);
        if outcome.is_none_or(|outcome| outcome == HitOutcome::Rejected) {
            continue;
        }
        state.rest = Rest::Awake;
        if state.living.is_some_and(|living| !living.is_alive()) {
            dead.push(own);
        }
    }
}

fn overcrowded(candidate: &CrowdCandidate, dead: &[GridRow], limit: i64) -> bool {
    let own = &candidate.own;
    let bounds = own.bounds();
    let mut count = i64::from(candidate.candidates);
    let mut free = i64::from(candidate.free);
    for gone in dead {
        if pushes(own, gone) && gone.bounds().intersects(&bounds) {
            count -= 1;
            free -= i64::from(!gone.flags.contains(GridFlags::IS_PASSENGER));
        }
    }
    count > limit && free > limit
}
