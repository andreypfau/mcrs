use glam::DVec3;
use mcrs_minecraft_brain::Applied;
use mcrs_minecraft_brain::Intents;

use crate::BodyFlags;
use crate::BodyRotation;
use crate::ControlInput;
use crate::ControlOutput;
use crate::ControlSteering;
use crate::Follower;
use crate::LookControl;
use crate::LookDefaults;
use crate::MobPassenger;
use crate::MoveControl;
use crate::Rotations;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MobControl {
    defaults: LookDefaults,
    follower: Follower,
    movement: MoveControl,
    look: LookControl,
    body: BodyRotation,
}

const DECAY: f32 = 0.98;

impl MobControl {
    #[must_use]
    pub fn new(defaults: LookDefaults) -> Self {
        Self {
            defaults,
            follower: Follower::default(),
            movement: MoveControl::default(),
            look: LookControl::default(),
            body: BodyRotation::default(),
        }
    }

    #[must_use]
    pub const fn applied(&self) -> Applied {
        self.follower.applied()
    }

    #[must_use]
    pub const fn has_path(&self) -> bool {
        self.follower.is_running()
    }

    pub fn apply(&mut self, intents: &Intents, position: DVec3) {
        if let Some(path) = &intents.path {
            self.follower.apply(path, position);
        }
        if let Some(look) = &intents.look {
            self.look.apply(look);
        }
    }

    pub fn before_travel(&mut self, input: &ControlInput) -> ControlOutput {
        let followed = self.follower.step(input);
        if let Some(wanted) = followed.wanted {
            self.movement.set_wanted(wanted);
        }
        let moved = self.movement.step(input);
        let yaw = moved.yaw.unwrap_or(input.rotations.yaw);
        let head = self.look.step(
            input,
            &self.defaults,
            input.rotations.head_yaw,
            self.follower.is_running(),
        );
        ControlOutput {
            steering: Some(ControlSteering {
                sideways: input.sideways * DECAY,
                vertical: 0.0,
                forward: moved.forward,
                jump: moved.jump,
                speed: moved.speed.unwrap_or(input.speed),
            }),
            yaw: moved.yaw.map(|_| yaw),
            pitch: Some(head.pitch),
            head_yaw: Some(head.head_yaw),
            body_yaw: None,
            path_ended: followed.ended,
        }
    }

    #[must_use]
    pub fn after_travel(
        &mut self,
        input: &ControlInput,
        end: DVec3,
        mut rotations: Rotations,
    ) -> Rotations {
        let passenger = if input.flags.contains(BodyFlags::MOB_PASSENGER) {
            MobPassenger::Yes
        } else {
            MobPassenger::No
        };
        self.body.step(
            end.x - input.position.x,
            end.z - input.position.z,
            passenger,
            input.body.head_yaw_limit,
            &mut rotations,
        );
        rotations
    }
}
