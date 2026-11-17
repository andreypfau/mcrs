use glam::DVec3;
use mcrs_minecraft_physics::MotionAttributes;
use mcrs_minecraft_physics::MotionEffects;
use mcrs_minecraft_physics::PlayerState;
use mcrs_minecraft_physics::SprintSpeed;
use mcrs_voxel::BlockPos;
use mcrs_voxel::BlockStateId;

#[derive(Clone, Copy, Debug, PartialEq)]
#[non_exhaustive]
pub enum Change {
    Motion(DVec3),
    Impulse(DVec3),
    Abilities {
        may_fly: bool,
        flying: bool,
        flight_speed: f32,
    },
    Attributes {
        attributes: MotionAttributes,
        sprint_speed: SprintSpeed,
    },
    Effects(MotionEffects),
    Block {
        at: BlockPos,
        old: BlockStateId,
        new: BlockStateId,
    },
    BlockAck,
}

impl Change {
    pub(crate) fn apply(&self, state: &mut PlayerState) {
        match *self {
            Change::Motion(velocity) => state.body.velocity = velocity,
            Change::Impulse(impulse) => state.body.velocity += impulse,
            Change::Abilities {
                may_fly,
                flying,
                flight_speed,
            } => {
                state.may_fly = may_fly;
                state.flight0 = flying;
                state.flight_speed = flight_speed;
            }
            Change::Attributes {
                attributes,
                sprint_speed,
            } => {
                state.attributes = attributes;
                state.sprint_speed = sprint_speed;
            }
            Change::Effects(effects) => state.effects = effects,
            Change::Block { .. } | Change::BlockAck => {}
        }
    }
}
