use bevy_ecs::change_detection::{DetectChangesMut, Mut};
use bevy_ecs::query::QueryData;
use glam::DVec3;
use mcrs_minecraft_engine_entity_motion::MotionState;
use mcrs_minecraft_entity_visual::{ClientEntity, Motion, PacketPath, Placement, Rotation};

use crate::orb_interpolation::OrbInterpolation;
use crate::velocity::Velocity;

/// A client entity that a packet moves. The position and the velocity of a simulated entity are
/// in its [`MotionState`]; every other entity keeps them in [`Motion`] and [`Velocity`].
#[derive(QueryData)]
#[query_data(mutable)]
pub(crate) struct MovedEntity {
    pub(crate) client: &'static ClientEntity,
    pub(crate) motion: &'static mut Motion,
    simulated: Option<&'static mut MotionState>,
    orb: Option<&'static mut OrbInterpolation>,
    velocity: Option<&'static mut Velocity>,
}

fn set_position(state: &mut Mut<MotionState>, position: DVec3) {
    state
        .reborrow()
        .map_unchanged(|state| &mut state.0.body.position)
        .set_if_neq(position);
}

impl MovedEntityItem<'_, '_> {
    pub(crate) fn current(&self) -> Placement {
        let mut current = self.motion.current;
        if let Some(state) = &self.simulated {
            current.position = state.0.body.position;
        }
        current
    }

    /// Moves the entity towards the end of `path` and `rotation` without a snap: a living entity
    /// starts an interpolation, an orb starts its own, and another simulated entity is there at
    /// once and keeps its previous placement.
    pub(crate) fn update(&mut self, path: Option<PacketPath>, rotation: Option<Rotation>) {
        let current = self.current();
        let end = path.as_ref().and_then(PacketPath::end);
        match (self.simulated.as_mut(), self.orb.as_mut()) {
            (Some(_), Some(orb)) => orb.start(current, end, rotation),
            (Some(state), None) => {
                if let Some(end) = end {
                    set_position(state, end);
                }
                if let Some(rotation) = rotation {
                    self.motion.current.yaw = rotation.yaw;
                    self.motion.current.pitch = rotation.pitch;
                }
            }
            (None, _) => {
                let length = self.client.update_interval();
                self.motion.start(path, rotation, length);
            }
        }
    }

    /// Puts the entity at `placement` with its previous position at `previous_position` and its
    /// previous rotation at the new rotation. A path or an orb interpolation stays.
    pub(crate) fn snap(&mut self, placement: Placement, previous_position: DVec3) {
        if let Some(state) = self.simulated.as_mut() {
            set_position(state, placement.position);
            self.motion.current.yaw = placement.yaw;
            self.motion.current.pitch = placement.pitch;
        } else {
            self.motion.current = placement;
        }
        self.motion.previous = Placement::new(previous_position, placement.yaw, placement.pitch);
    }

    pub(crate) fn set_on_ground(&mut self, on_ground: bool) {
        match self.simulated.as_mut() {
            Some(state) => {
                state
                    .reborrow()
                    .map_unchanged(|state| &mut state.0.body.flags.on_ground)
                    .set_if_neq(on_ground);
            }
            None => self.motion.on_ground = on_ground,
        }
    }

    pub(crate) fn velocity(&self) -> DVec3 {
        match (&self.simulated, &self.velocity) {
            (Some(state), _) => state.0.body.velocity,
            (None, Some(velocity)) => velocity.value,
            (None, None) => DVec3::ZERO,
        }
    }

    /// Sets the velocity at once. An approach of another player continues from the new value.
    pub(crate) fn set_velocity(&mut self, value: DVec3) {
        match (self.simulated.as_mut(), self.velocity.as_mut()) {
            (Some(state), _) => {
                state
                    .reborrow()
                    .map_unchanged(|state| &mut state.0.body.velocity)
                    .set_if_neq(value);
            }
            (None, Some(velocity)) => {
                velocity
                    .reborrow()
                    .map_unchanged(|velocity| &mut velocity.value)
                    .set_if_neq(value);
            }
            (None, None) => {}
        }
    }

    /// Starts the approach of the velocity of another player to `target`.
    pub(crate) fn approach_velocity(&mut self, target: DVec3) {
        if let Some(velocity) = self.velocity.as_mut() {
            let next = velocity.approaching(target);
            velocity.set_if_neq(next);
        }
    }
}
