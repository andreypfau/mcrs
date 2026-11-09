use mcrs_minecraft_physics::HitRecord;
use mcrs_minecraft_physics::Living;

use crate::Hit;
use crate::HitOutcome;

/// The damage cooldown above which a hit is a partial hit (DMG-HIT-003).
const PARTIAL_ABOVE: u8 = 10;
const COOLDOWN: u8 = 20;
const HURT_TIME: u8 = 10;

/// Does `hit` in tick `tick` on the entity of `living` and `record` (DMG-HIT-001 to DMG-HIT-005,
/// DMG-DTH-001). A dying entity rejects the hit and records nothing. Each other hit records the
/// strike for the idle time, also one that the cooldown rejects; an accepted hit becomes the last
/// hit of `record`. An accepted hit that takes the health to 0 starts the death.
pub fn hurt(living: &mut Living, record: &mut HitRecord, hit: Hit, tick: u64) -> HitOutcome {
    if !living.is_alive() {
        return HitOutcome::Rejected;
    }
    record.strike();
    let outcome = apply(living, hit.amount);
    if outcome == HitOutcome::Rejected {
        return outcome;
    }
    if living.health <= 0.0 {
        living.death_time = Some(0);
    }
    record.accept(tick, hit.damage_type);
    outcome
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
        let mut living = pig();
        let mut record = HitRecord::default();
        let drown = Hit::new(0.0, 3);
        assert_eq!(hurt(&mut living, &mut record, drown, 10), HitOutcome::Full);
        assert_eq!(
            hurt(&mut living, &mut record, drown, 12),
            HitOutcome::Rejected,
            "the cooldown runs"
        );
        assert_eq!(record.strikes(), 2, "the rejected hit strikes the pig");
        assert_eq!(
            record.last().map(|hit| hit.tick),
            Some(10),
            "the damage record keeps the accepted hit"
        );

        living.death_time = Some(1);
        assert_eq!(
            hurt(&mut living, &mut record, drown, 40),
            HitOutcome::Rejected
        );
        assert_eq!(record.strikes(), 2, "a hit on a dying pig strikes nothing");
    }

    #[test]
    fn dth_004_a_dying_mob_rejects_each_hit() {
        let mut living = pig();
        living.death_time = Some(3);
        assert_eq!(apply(&mut living, f32::MAX), HitOutcome::Rejected);
    }
}
