use bevy_ecs::entity::Entity;
use mcrs_minecraft_damage::Hit;
use mcrs_minecraft_damage::HitOutcome;
use mcrs_minecraft_entity_key::TileKey;
use mcrs_minecraft_physics::EntityState;

use crate::mob_changes::MobChanges;
use crate::mob_changes::Struck;

pub fn hurt_mob(
    entity: Entity,
    state: &mut EntityState,
    changes: &mut MobChanges,
    hit: Hit,
    tick: u64,
) -> Option<HitOutcome> {
    let living = state.living.as_mut()?;
    let outcome = mcrs_minecraft_damage::hurt(living, &mut state.hits, hit, tick);
    if outcome == HitOutcome::Rejected {
        return Some(outcome);
    }
    changes.strikes.push(Struck {
        entity,
        uuid: state.key,
        tile: TileKey::at(state.body.position),
        damage_type: hit.damage_type,
        outcome,
        died: !living.is_alive(),
    });
    Some(outcome)
}
