use bevy_ecs::entity::Entity;
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
    swing: &mut MobSwing,
    changes: &mut MobChanges,
    spec: MobSwingSpec,
    tick: u64,
) -> bool {
    let Some(living) = state.living.as_ref() else {
        return false;
    };
    if living
        .death_time
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

    fn alive_state() -> EntityState {
        EntityState::new(
            EntityUuid::from_u128(1),
            Body {
                position: DVec3::new(3.5, 64.0, 3.5),
                ..Body::default()
            },
            Some(Living {
                health: 10.0,
                ..Living::default()
            }),
        )
    }

    fn dying_state(death_time: u8) -> EntityState {
        EntityState::new(
            EntityUuid::from_u128(1),
            Body {
                position: DVec3::new(3.5, 64.0, 3.5),
                ..Body::default()
            },
            Some(Living {
                health: 0.0,
                death_time: Some(death_time),
                ..Living::default()
            }),
        )
    }

    #[test]
    fn swing_mob_checks_living_state_cooldown_and_modifier_scaling() {
        let entity = Entity::from_raw_u32(7).expect("entity");
        let non_living = EntityState::new(EntityUuid::from_u128(2), Body::default(), None);
        let alive = alive_state();
        let mut swing = MobSwing::default();
        let mut changes = MobChanges::default();
        let default_spec = MobSwingSpec::new(Hand::Main, SwingAnimationKind::Whack, 6);

        assert!(
            !swing_mob(
                entity,
                &non_living,
                &mut swing,
                &mut changes,
                default_spec,
                10
            ),
            "non-living state must not swing"
        );
        let removed = dying_state(CORPSE_REMOVAL_TICKS);
        assert!(
            !swing_mob(entity, &removed, &mut swing, &mut changes, default_spec, 10),
            "a mob at corpse removal must not swing"
        );
        assert!(changes.swings().is_empty(), "rejected swings push nothing");

        let dying = dying_state(CORPSE_REMOVAL_TICKS - 1);
        let mut dying_swing = MobSwing::default();
        assert!(
            swing_mob(
                entity,
                &dying,
                &mut dying_swing,
                &mut changes,
                default_spec,
                10
            ),
            "a dying mob swings until corpse removal"
        );
        changes = MobChanges::default();

        let haste = MobSwingSpec::new(Hand::Off, SwingAnimationKind::Stab, 10)
            .with_modifiers(SwingModifiers::from_effects([("minecraft:haste", 2)]));
        assert!(
            swing_mob(entity, &alive, &mut swing, &mut changes, haste, 10),
            "alive mob starts swing"
        );
        assert!(
            !swing_mob(entity, &alive, &mut swing, &mut changes, default_spec, 10),
            "SWG-STA-004: same-tick second swing is suppressed"
        );
        assert!(
            !swing_mob(entity, &alive, &mut swing, &mut changes, default_spec, 13),
            "CMD-SWG-006: haste 2 scales B=10 to D=7 blocking T+3"
        );
        assert!(
            swing_mob(entity, &alive, &mut swing, &mut changes, default_spec, 14),
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
