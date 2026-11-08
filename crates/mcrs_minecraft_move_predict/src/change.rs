use glam::DVec3;
use mcrs_minecraft_physics::MotionAttributes;
use mcrs_minecraft_physics::MotionEffects;
use mcrs_minecraft_physics::PlayerState;
use mcrs_voxel::BlockPos;
use mcrs_voxel::BlockStateId;

/// A clientbound packet that changes the prediction of the player, as the client applies it.
#[derive(Clone, Copy, Debug, PartialEq)]
#[non_exhaustive]
pub enum Change {
    /// `set_entity_motion` for the player, with the velocity that the client decodes.
    Motion(DVec3),
    /// The impulse of `explode`, which adds to the velocity.
    Impulse(DVec3),
    /// The clientbound `player_abilities`.
    Abilities {
        may_fly: bool,
        flying: bool,
        flight_speed: f32,
    },
    /// The attributes after an `update_attributes`, without the sprint modifier.
    Attributes(MotionAttributes),
    /// The effects after an `update_mob_effect` or a `remove_mob_effect`.
    Effects(MotionEffects),
    /// A block that the server sends: `block_update` or one entry of `section_blocks_update`.
    Block {
        at: BlockPos,
        old: BlockStateId,
        new: BlockStateId,
    },
    /// `block_changed_ack`: the client stops to keep the server states of its own edits.
    BlockAck,
}

impl Change {
    /// Applies the change to the state of one candidate. A block changes the world view instead.
    pub(crate) fn apply(&self, state: &mut PlayerState) {
        match *self {
            Change::Motion(velocity) => state.velocity = velocity,
            Change::Impulse(impulse) => state.velocity += impulse,
            Change::Abilities {
                may_fly,
                flying,
                flight_speed,
            } => {
                state.may_fly = may_fly;
                state.flight0 = flying;
                state.flight_speed = flight_speed;
            }
            Change::Attributes(attributes) => state.attributes = attributes,
            Change::Effects(effects) => state.effects = effects,
            Change::Block { .. } | Change::BlockAck => {}
        }
    }
}
