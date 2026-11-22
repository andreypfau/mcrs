use bevy_ecs::entity::Entity;
use mcrs_minecraft_damage::Vitals;
use mcrs_minecraft_physics::CORPSE_REMOVAL_TICKS;
use mcrs_minecraft_physics::EntityState;
use mcrs_minecraft_protocol_game::{Hand, SwingAnimationKind};
use mcrs_minecraft_swing::SwingModifiers;

use crate::mob_changes::{MobChanges, Swung};
use crate::mob_swing::MobSwing;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct MobSwingSpec {
    pub hand: Hand,
    pub animation: SwingAnimationKind,
    pub duration: i32,
    pub modifiers: SwingModifiers,
}

impl MobSwingSpec {
    #[must_use]
    pub const fn new(hand: Hand, animation: SwingAnimationKind, duration: i32) -> Self {
        Self {
            hand,
            animation,
            duration,
            modifiers: SwingModifiers {
                haste: None,
                conduit_power: None,
                mining_fatigue: None,
            },
        }
    }

    #[must_use]
    pub const fn with_modifiers(mut self, modifiers: SwingModifiers) -> Self {
        self.modifiers = modifiers;
        self
    }
}

pub fn swing_mob(
    entity: Entity,
    state: &EntityState,
    vitals: Option<&Vitals>,
    swing: &mut MobSwing,
    changes: &mut MobChanges,
    spec: MobSwingSpec,
    tick: u64,
) -> bool {
    if state.living.is_none() {
        return false;
    }
    if vitals
        .and_then(|vitals| vitals.death_time)
        .is_some_and(|death_time| death_time >= CORPSE_REMOVAL_TICKS)
    {
        return false;
    }
    let base_duration = spec.duration.max(0);
    if swing
        .0
        .try_start(tick, base_duration, spec.modifiers)
        .is_none()
    {
        return false;
    }
    changes.swings.push(Swung {
        entity,
        hand: spec.hand,
        animation: spec.animation,
        duration: base_duration,
    });
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::DVec3;
    use mcrs_minecraft_entity_key::EntityUuid;
    use mcrs_minecraft_physics::{Body, Living};

    fn living_state() -> EntityState {
        EntityState::new(
            EntityUuid::from_u128(1),
            Body {
                position: DVec3::new(3.5, 64.0, 3.5),
                ..Body::default()
            },
            Some(Living::default()),
        )
    }

    fn dying_vitals(death_time: u8) -> Vitals {
        Vitals {
            health: 0.0,
            death_time: Some(death_time),
            ..Vitals::default()
        }
    }

    #[test]
    fn swing_mob_checks_living_and_corpse_removal_eligibility() {
        let entity = Entity::from_raw_u32(7).expect("entity");
        let non_living = EntityState::new(EntityUuid::from_u128(2), Body::default(), None);
        let living = living_state();
        let default_spec = MobSwingSpec::new(Hand::Main, SwingAnimationKind::Whack, 6);
        let removed = dying_vitals(CORPSE_REMOVAL_TICKS);
        let dying = dying_vitals(CORPSE_REMOVAL_TICKS - 1);
        let cases = [
            (&non_living, None, false, "non-living state must not swing"),
            (
                &living,
                Some(&removed),
                false,
                "a mob at corpse removal must not swing",
            ),
            (
                &living,
                Some(&dying),
                true,
                "a dying mob swings until corpse removal",
            ),
            (&living, None, true, "a living mob without vitals can swing"),
        ];
        for (state, vitals, expected, reason) in cases {
            let mut swing = MobSwing::default();
            let mut changes = MobChanges::default();
            assert_eq!(
                swing_mob(
                    entity,
                    state,
                    vitals,
                    &mut swing,
                    &mut changes,
                    default_spec,
                    10
                ),
                expected,
                "{reason}"
            );
            assert_eq!(changes.swings().len(), usize::from(expected), "{reason}");
        }
    }

    #[test]
    fn swing_mob_enforces_cooldown_and_modifier_scaling() {
        let entity = Entity::from_raw_u32(7).expect("entity");
        let living = living_state();
        let alive = Vitals::new(10.0);
        let mut swing = MobSwing::default();
        let mut changes = MobChanges::default();
        let default_spec = MobSwingSpec::new(Hand::Main, SwingAnimationKind::Whack, 6);
        let haste = MobSwingSpec::new(Hand::Off, SwingAnimationKind::Stab, 10)
            .with_modifiers(SwingModifiers::from_effects([("minecraft:haste", 2)]));

        assert!(
            swing_mob(
                entity,
                &living,
                Some(&alive),
                &mut swing,
                &mut changes,
                haste,
                10
            ),
            "alive mob starts swing"
        );
        assert!(
            !swing_mob(
                entity,
                &living,
                Some(&alive),
                &mut swing,
                &mut changes,
                default_spec,
                10
            ),
            "SWG-STA-004: same-tick second swing is suppressed"
        );
        assert!(
            !swing_mob(
                entity,
                &living,
                Some(&alive),
                &mut swing,
                &mut changes,
                default_spec,
                13
            ),
            "CMD-SWG-006: haste 2 scales B=10 to D=7 blocking T+3"
        );
        assert!(
            swing_mob(
                entity,
                &living,
                Some(&alive),
                &mut swing,
                &mut changes,
                default_spec,
                14
            ),
            "CMD-SWG-006: D=7 unlocks at T+4"
        );
        assert_eq!(
            changes.swings(),
            [
                Swung {
                    entity,
                    hand: Hand::Off,
                    animation: SwingAnimationKind::Stab,
                    duration: 10,
                },
                Swung {
                    entity,
                    hand: Hand::Main,
                    animation: SwingAnimationKind::Whack,
                    duration: 6,
                },
            ],
            "wire duration stays unscaled"
        );
    }
}
