use mcrs_minecraft_physics::Living;
use mcrs_minecraft_replication::MobSignal;

use crate::entity_rows::EntityRows;
use crate::mob_life::MobChanges;
use crate::mob_life::Placed;
use crate::mob_life::Signaled;
use crate::mobs::MobHit;
use crate::mobs::Mobs;

/// The damage cooldown above which a hit is a partial hit (DMG-HIT-003).
const PARTIAL_ABOVE: u8 = 10;
const COOLDOWN: u8 = 20;
const HURT_TIME: u8 = 10;

/// A hit on a mob: the amount and the network id of the damage type. The source has no entity
/// and no position.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hit {
    pub amount: f32,
    pub damage_type: u32,
}

/// What a hit did (DMG-HIT-003).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum HitOutcome {
    Rejected,
    Partial,
    Full,
}

/// Does `hit` on the mob row `at` in tick `tick` (DMG-HIT-001 to DMG-HIT-005, DMG-DTH-001), and
/// records what its viewers see. A row that is gone or dying rejects the hit; each other hit
/// records the strike for the idle time, also one that the cooldown rejects. Gives `None` when
/// `at` is not a living row.
pub fn hurt(
    rows: &mut EntityRows,
    mobs: &mut Mobs,
    changes: &mut MobChanges,
    at: Placed,
    hit: Hit,
    tick: u64,
) -> Option<HitOutcome> {
    let living = rows.state_mut(at)?.living.as_mut()?;
    if !living.is_alive() {
        return Some(HitOutcome::Rejected);
    }
    if let Some(mob) = mobs.0.get_mut(&at.id) {
        mob.strikes = mob.strikes.wrapping_add(1);
    }
    let outcome = apply(living, hit.amount);
    if outcome == HitOutcome::Rejected {
        return Some(outcome);
    }
    let died = living.health <= 0.0 && living.is_alive();
    if died {
        living.death_time = Some(0);
    }
    if let Some(mob) = mobs.0.get_mut(&at.id) {
        mob.last_hit = Some(MobHit {
            tick,
            damage_type: hit.damage_type,
        });
    }
    let mut signal = |signal| changes.signals.push(Signaled { at, signal });
    if outcome == HitOutcome::Full {
        signal(MobSignal::Hurt {
            damage_type: hit.damage_type,
        });
    }
    if died {
        signal(MobSignal::Died);
    }
    changes.touched.push(at);
    Some(outcome)
}

/// What `amount` would do to `living`, which stays as it is.
#[must_use]
pub fn preview(living: &Living, amount: f32) -> HitOutcome {
    apply(&mut living.clone(), amount)
}

/// The cooldown and the health change of one hit. A dying entity rejects it.
fn apply(living: &mut Living, amount: f32) -> HitOutcome {
    if !living.is_alive() {
        return HitOutcome::Rejected;
    }
    let amount = if amount.is_finite() {
        amount.max(0.0)
    } else {
        f32::MAX
    };
    let (outcome, damage) = if living.damage_cooldown > PARTIAL_ABOVE {
        if amount <= living.last_damage {
            return HitOutcome::Rejected;
        }
        (HitOutcome::Partial, amount - living.last_damage)
    } else {
        living.damage_cooldown = COOLDOWN;
        living.hurt_time = HURT_TIME;
        (HitOutcome::Full, amount)
    };
    living.last_damage = amount;
    if damage != 0.0 {
        living.health = (living.health - damage).max(0.0);
    }
    outcome
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pig() -> Living {
        Living {
            health: 10.0,
            ..Living::default()
        }
    }

    /// CMD-DMG-003: a hit of 0 is a full hit with no health change, and a second one within 10
    /// ticks is rejected; a larger one in the cooldown applies only the difference.
    #[test]
    fn hit_003_the_cooldown_rejects_an_equal_hit_and_applies_the_rest_of_a_larger_one() {
        let mut living = pig();
        assert_eq!(apply(&mut living, 0.0), HitOutcome::Full);
        assert_eq!(
            (living.health, living.hurt_time, living.damage_cooldown),
            (10.0, 10, 20)
        );
        assert_eq!(apply(&mut living, 0.0), HitOutcome::Rejected);
        assert_eq!(apply(&mut living, 3.0), HitOutcome::Partial);
        assert_eq!(living.health, 7.0);
        living.damage_cooldown = 10;
        assert_eq!(
            apply(&mut living, 1.0),
            HitOutcome::Full,
            "the cooldown ended"
        );
        assert_eq!(living.health, 6.0);
    }

    /// DMG-HIT-002 step 1, GOAL-STR-004: a hit that the cooldown rejects records the strike that
    /// resets the idle time, but not the damage record; a hit on a dying mob records neither.
    #[test]
    fn hit_002_a_hit_that_the_cooldown_rejects_still_strikes_the_mob() {
        use glam::DVec3;
        use mcrs_engine_muxer::EntityId;
        use mcrs_minecraft_mob::MobType;
        use mcrs_minecraft_physics::Birth;
        use mcrs_minecraft_physics::BirthKey;
        use mcrs_minecraft_physics::SourceKind;

        use crate::mob_life::mob_body;
        use crate::mob_life::mob_row;
        use crate::mobs::Mob;
        use crate::tile_at::tile_at;

        let id = EntityId(1);
        let position = DVec3::new(0.5, 0.0, 0.5);
        let row = mob_row(MobType::Pig);
        let key = Birth::from(BirthKey::new(0, SourceKind::Command, 0, 0));
        let mut rows = EntityRows::default();
        rows.spawn(id, mob_body(MobType::Pig, &row, key, position), row);
        let mut mobs = Mobs::default();
        mobs.0.insert(
            id,
            Mob {
                kind: MobType::Pig,
                uuid: 1,
                looks: Default::default(),
                tile: tile_at(position),
                custom_name: None,
                tags: Vec::new(),
                last_hit: None,
                strikes: 0,
            },
        );
        let mut changes = MobChanges::default();
        let at = Placed {
            tile: tile_at(position),
            id,
        };
        let drown = Hit {
            amount: 0.0,
            damage_type: 3,
        };
        let mut hit = |tick, hit| hurt(&mut rows, &mut mobs, &mut changes, at, hit, tick);
        assert_eq!(hit(10, drown), Some(HitOutcome::Full));
        assert_eq!(
            hit(12, drown),
            Some(HitOutcome::Rejected),
            "the cooldown runs"
        );
        let mob = mobs.0.get(&id).expect("the pig keeps its mob");
        assert_eq!(mob.strikes, 2, "the rejected hit strikes the pig");
        assert_eq!(
            mob.last_hit.map(|hit| hit.tick),
            Some(10),
            "the damage record keeps the accepted hit"
        );

        if let Some(living) = rows.state_mut(at).and_then(|state| state.living.as_mut()) {
            living.death_time = Some(1);
        }
        let mut hit = |tick, hit| hurt(&mut rows, &mut mobs, &mut changes, at, hit, tick);
        assert_eq!(hit(40, drown), Some(HitOutcome::Rejected));
        assert_eq!(
            mobs.0.get(&id).map(|mob| mob.strikes),
            Some(2),
            "a hit on a dying pig strikes nothing"
        );
    }

    #[test]
    fn dth_004_a_dying_mob_rejects_each_hit() {
        let mut living = pig();
        living.death_time = Some(3);
        assert_eq!(apply(&mut living, f32::MAX), HitOutcome::Rejected);
    }
}
