use bevy_ecs::prelude::*;
use glam::DVec3;
use mcrs_minecraft_engine_entity_motion::MotionState;
use mcrs_minecraft_entity::entity_type_keys::EntityType;
use mcrs_minecraft_entity_visual::{Age, ClientEntity, Living, Motion, TickVelocity};

use crate::ticked::ticked;

const DECAY: f64 = 0.98;
const CUTOFF: f64 = 0.003;
const APPROACH_TICKS: u8 = 3;

#[derive(Component, Clone, Copy, Debug, Default, PartialEq)]
pub struct TrackedVelocity {
    pub value: DVec3,
    pub(crate) approach: Option<Approach>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Approach {
    target: DVec3,
    remaining_ticks: u8,
}

impl TrackedVelocity {
    #[must_use]
    pub const fn new(value: DVec3) -> Self {
        Self {
            value,
            approach: None,
        }
    }

    #[must_use]
    pub fn approaching(self, target: DVec3) -> Self {
        Self {
            approach: Some(Approach {
                target,
                remaining_ticks: APPROACH_TICKS,
            }),
            ..self
        }
    }

    #[must_use]
    pub fn approached(self) -> Self {
        let Some(Approach {
            target,
            remaining_ticks,
        }) = self.approach
        else {
            return self;
        };
        let value = self.value + (target - self.value) / f64::from(remaining_ticks);
        let remaining_ticks = remaining_ticks.saturating_sub(1);
        Self {
            value,
            approach: (remaining_ticks > 0).then_some(Approach {
                target,
                remaining_ticks,
            }),
        }
    }

    #[must_use]
    pub fn decayed(self) -> Self {
        let cut = |axis: f64| if axis.abs() < CUTOFF { 0.0 } else { axis };
        let value = self.value * DECAY;
        Self {
            value: DVec3::new(cut(value.x), cut(value.y), cut(value.z)),
            ..self
        }
    }
}

#[expect(
    clippy::type_complexity,
    reason = "one query reads each part of an entity that its velocity tick needs"
)]
pub(crate) fn tick_velocities(
    mut entities: Populated<
        (
            &ClientEntity,
            Ref<Age>,
            &Motion,
            &mut TrackedVelocity,
            Option<&mut TickVelocity>,
        ),
        (With<Living>, Without<MotionState>),
    >,
) {
    for (client, age, motion, mut velocity, tick_velocity) in &mut entities {
        if !ticked(&age) {
            continue;
        }
        let next = if client.kind == EntityType::Player {
            Some(velocity.approached())
        } else if motion.interpolation.is_active() {
            None
        } else {
            Some(velocity.decayed())
        };
        if let Some(next) = next {
            velocity.set_if_neq(next);
        }
        if let Some(mut tick_velocity) = tick_velocity {
            tick_velocity.advance(velocity.value);
        }
    }
}
