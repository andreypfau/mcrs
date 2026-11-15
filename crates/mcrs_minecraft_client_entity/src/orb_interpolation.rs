use bevy_ecs::prelude::*;
use glam::DVec3;
use mcrs_minecraft_engine_entity_motion::MotionState;
use mcrs_minecraft_entity_visual::{Age, Motion, Placement, Rotation, angle};

use crate::velocity::ticked;

const STEPS: u8 = 3;
/// A movement from another cause smaller than this, squared, does not move the target.
const MIN_FOLLOW_DISTANCE_SQUARED: f64 = 1e-5;

/// The movement of an experience orb towards the placement of the last update of the server, in
/// a fixed number of steps.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq)]
pub struct OrbInterpolation {
    target: Placement,
    remaining_steps: u8,
    /// The placement of the orb at the end of the latest start or step.
    last: Placement,
}

impl OrbInterpolation {
    #[must_use]
    pub fn is_active(&self) -> bool {
        self.remaining_steps > 0
    }

    /// Starts the movement to `position` and `rotation`; each one that is missing keeps the value
    /// of the active target, or of `current` when no movement is active.
    pub fn start(
        &mut self,
        current: Placement,
        position: Option<DVec3>,
        rotation: Option<Rotation>,
    ) {
        let reference = if self.is_active() {
            self.target
        } else {
            current
        };
        let rotation = rotation.unwrap_or(reference.rotation());
        let target = Placement::new(
            position.unwrap_or(reference.position),
            rotation.yaw,
            rotation.pitch,
        );
        if self.is_active() && self.target == target {
            return;
        }
        self.target = target;
        self.remaining_steps = STEPS;
        self.last = current;
    }

    /// The placement after one step from `current`. The target first moves with any change of
    /// `current` since the last step.
    #[must_use]
    pub fn step(&mut self, current: Placement) -> Placement {
        if !self.is_active() {
            return current;
        }
        let moved = current.position - self.last.position;
        if moved.length_squared() > MIN_FOLLOW_DISTANCE_SQUARED {
            self.target.position += moved;
        }
        self.target.yaw += current.yaw - self.last.yaw;
        self.target.pitch += current.pitch - self.last.pitch;
        let fraction = 1.0 / f64::from(self.remaining_steps);
        let angle_fraction = fraction as f32;
        let next = Placement::new(
            current.position.lerp(self.target.position, fraction),
            angle::alerp(angle_fraction, current.yaw, self.target.yaw),
            current.pitch + angle_fraction * (self.target.pitch - current.pitch),
        );
        self.remaining_steps = self.remaining_steps.saturating_sub(1);
        self.last = next;
        next
    }
}

/// Steps the interpolation of each orb that ticked. It writes the position into the
/// [`MotionState`] and the rotation into the [`Motion`].
pub(crate) fn step_orb_interpolations(
    mut orbs: Query<(
        Ref<Age>,
        &mut Motion,
        &mut MotionState,
        &mut OrbInterpolation,
    )>,
) {
    for (age, mut motion, state, mut orb) in &mut orbs {
        if !ticked(&age) || !orb.is_active() {
            continue;
        }
        let current = Placement::new(
            state.0.body.position,
            motion.current.yaw,
            motion.current.pitch,
        );
        let next = orb.step(current);
        state
            .map_unchanged(|state| &mut state.0.body.position)
            .set_if_neq(next.position);
        motion.current.yaw = next.yaw;
        motion.current.pitch = next.pitch;
    }
}
