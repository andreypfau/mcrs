use bevy_ecs::change_detection::{DetectChangesMut, Mut};
use bevy_ecs::query::QueryData;
use glam::DVec3;
use mcrs_minecraft_engine_entity_motion::MotionState;
use mcrs_minecraft_entity_visual::{ClientEntity, Motion, PacketPath, Placement, Rotation};

use crate::orb_interpolation::OrbInterpolation;
use crate::tracked_velocity::TrackedVelocity;

#[derive(QueryData)]
#[query_data(mutable)]
pub(crate) struct MovedEntity {
    pub(crate) client: &'static ClientEntity,
    pub(crate) motion: &'static mut Motion,
    simulated: Option<&'static mut MotionState>,
    orb: Option<&'static mut OrbInterpolation>,
    velocity: Option<&'static mut TrackedVelocity>,
}

fn set_field<T, V: PartialEq>(
    component: Mut<'_, T>,
    field: impl FnOnce(&mut T) -> &mut V,
    value: V,
) {
    component.map_unchanged(field).set_if_neq(value);
}

impl MovedEntityItem<'_, '_> {
    pub(crate) fn current(&self) -> Placement {
        let mut current = self.motion.current;
        if let Some(state) = &self.simulated {
            current.position = state.0.body.position;
        }
        current
    }

    pub(crate) fn reference(&self) -> Placement {
        let current = self.current();
        match (&self.simulated, &self.orb) {
            (Some(_), Some(orb)) => orb.reference(current),
            (Some(_), None) => current,
            (None, _) => self.motion.interpolation.reference(current),
        }
    }

    pub(crate) fn update(&mut self, path: Option<PacketPath>, rotation: Option<Rotation>) {
        let current = self.current();
        let end = path.as_ref().and_then(PacketPath::end);
        match (self.simulated.as_mut(), self.orb.as_mut()) {
            (Some(_), Some(orb)) => orb.start(current, end, rotation),
            (Some(state), None) => {
                if let Some(end) = end {
                    set_field(state.reborrow(), |state| &mut state.0.body.position, end);
                }
                if let Some(rotation) = rotation {
                    set_field(
                        self.motion.reborrow(),
                        |motion| &mut motion.current.yaw,
                        rotation.yaw,
                    );
                    set_field(
                        self.motion.reborrow(),
                        |motion| &mut motion.current.pitch,
                        rotation.pitch,
                    );
                }
            }
            (None, _) => {
                let length = self.client.update_interval();
                self.motion.start(path, rotation, length);
            }
        }
    }

    pub(crate) fn snap(&mut self, placement: Placement, previous_position: DVec3) {
        if let Some(state) = self.simulated.as_mut() {
            set_field(
                state.reborrow(),
                |state| &mut state.0.body.position,
                placement.position,
            );
            set_field(
                self.motion.reborrow(),
                |motion| &mut motion.current.yaw,
                placement.yaw,
            );
            set_field(
                self.motion.reborrow(),
                |motion| &mut motion.current.pitch,
                placement.pitch,
            );
        } else {
            set_field(
                self.motion.reborrow(),
                |motion| &mut motion.current,
                placement,
            );
        }
        set_field(
            self.motion.reborrow(),
            |motion| &mut motion.previous,
            Placement::new(previous_position, placement.yaw, placement.pitch),
        );
    }

    pub(crate) fn set_on_ground(&mut self, on_ground: bool) {
        match self.simulated.as_mut() {
            Some(state) => set_field(
                state.reborrow(),
                |state| &mut state.0.body.flags.on_ground,
                on_ground,
            ),
            None => set_field(
                self.motion.reborrow(),
                |motion| &mut motion.on_ground,
                on_ground,
            ),
        }
    }

    pub(crate) fn velocity(&self) -> DVec3 {
        match (&self.simulated, &self.velocity) {
            (Some(state), _) => state.0.body.velocity,
            (None, Some(velocity)) => velocity.value,
            (None, None) => DVec3::ZERO,
        }
    }

    pub(crate) fn set_velocity(&mut self, value: DVec3) {
        match (self.simulated.as_mut(), self.velocity.as_mut()) {
            (Some(state), _) => {
                set_field(state.reborrow(), |state| &mut state.0.body.velocity, value)
            }
            (None, Some(velocity)) => {
                set_field(velocity.reborrow(), |velocity| &mut velocity.value, value)
            }
            (None, None) => {}
        }
    }

    pub(crate) fn approach_velocity(&mut self, target: DVec3) {
        if let Some(velocity) = self.velocity.as_mut() {
            let next = velocity.approaching(target);
            velocity.set_if_neq(next);
        }
    }
}
