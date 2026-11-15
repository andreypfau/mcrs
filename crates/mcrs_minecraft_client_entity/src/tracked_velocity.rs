use bevy_ecs::prelude::*;
use glam::DVec3;
use mcrs_minecraft_engine_entity_motion::MotionState;
use mcrs_minecraft_entity::entity_type_keys::EntityType;
use mcrs_minecraft_entity_visual::{Age, ClientEntity, Living, Motion};

use crate::ticked::ticked;

const DECAY: f64 = 0.98;
const CUTOFF: f64 = 0.003;
const APPROACH_TICKS: u8 = 3;

/// The velocity of a client entity that the client does not simulate, in blocks for each tick.
/// It never moves the entity.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq)]
pub struct TrackedVelocity {
    pub value: DVec3,
    /// The packet value that the velocity of another player still approaches.
    pub(crate) approach: Option<Approach>,
}

/// A target velocity and the ticks left to reach it; each tick covers an even share of the
/// remaining difference.
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

    /// Starts the approach of another player to `target`; the value does not change yet.
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

    /// The velocity after one tick of the approach. With no approach it is unchanged.
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

    /// The velocity after one tick of decay of a living entity that is not a player.
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

/// Changes the velocity of each living entity that ticked: another player approaches the last
/// packet value, and any other living entity decays while it has no active interpolation.
pub(crate) fn tick_velocities(
    mut entities: Query<
        (&ClientEntity, Ref<Age>, &Motion, &mut TrackedVelocity),
        (With<Living>, Without<MotionState>),
    >,
) {
    for (client, age, motion, mut velocity) in &mut entities {
        if !ticked(&age) {
            continue;
        }
        let next = if client.kind == EntityType::Player {
            velocity.approached()
        } else if motion.interpolation.is_active() {
            continue;
        } else {
            velocity.decayed()
        };
        velocity.set_if_neq(next);
    }
}
