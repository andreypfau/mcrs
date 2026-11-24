use bevy_ecs::prelude::*;
use glam::DVec3;
use mcrs_minecraft_engine_entity_motion::MotionState;
use mcrs_minecraft_entity_visual::{Age, Motion, Placement, Rotation};
use mcrs_minecraft_numeric::wrap_degrees_f64;

use crate::ticked::ticked;

const STEPS: u8 = 3;
const MIN_FOLLOW_DISTANCE_SQUARED: f64 = 1e-5;

#[derive(Component, Clone, Copy, Debug, Default, PartialEq)]
pub struct OrbInterpolation {
    target: Placement,
    remaining_steps: u8,
    last: Placement,
}

impl OrbInterpolation {
    #[must_use]
    pub fn is_active(&self) -> bool {
        self.remaining_steps > 0
    }

    #[must_use]
    pub(crate) fn reference(&self, current: Placement) -> Placement {
        if self.is_active() {
            self.target
        } else {
            current
        }
    }

    pub(crate) fn start(
        &mut self,
        current: Placement,
        position: Option<DVec3>,
        rotation: Option<Rotation>,
    ) {
        let reference = self.reference(current);
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

    #[must_use]
    pub(crate) fn step(&mut self, current: Placement) -> Placement {
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
        let (yaw, pitch) = (f64::from(current.yaw), f64::from(current.pitch));
        let yaw = (yaw + fraction * wrap_degrees_f64(f64::from(self.target.yaw) - yaw)) as f32;
        let pitch = (pitch + fraction * (f64::from(self.target.pitch) - pitch)) as f32;
        let next = Placement::new(
            current.position.lerp(self.target.position, fraction),
            yaw % 360.0,
            (pitch % 360.0).clamp(-90.0, 90.0),
        );
        self.remaining_steps = self.remaining_steps.saturating_sub(1);
        self.last = next;
        next
    }
}

pub(crate) fn step_orb_interpolations(
    mut orbs: Populated<(
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
        if motion.current.yaw != next.yaw || motion.current.pitch != next.pitch {
            motion.current.yaw = next.yaw;
            motion.current.pitch = next.pitch;
        }
    }
}
