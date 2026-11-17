use mcrs_minecraft_pose::Dimensions;
use mcrs_minecraft_pose::Pose;

use crate::Aabb;
use crate::BodyView;
use crate::CollisionWorld;
use crate::QueryEntity;
use crate::Scratch;
use crate::View;
use crate::query;
use crate::tolerance::COLLISION;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PoseInputs {
    pub current: Pose,
    pub entity: QueryEntity,
    pub scale: f32,
    pub sleeping: bool,
    pub swimming: bool,
    pub fall_flying: bool,
    pub spin_attack: bool,
    pub shift: bool,
    pub flight: bool,
    pub spectator: bool,
    pub passenger: bool,
}

pub fn select_pose<W: CollisionWorld, B: BodyView>(
    view: View<'_, W, B>,
    inputs: &PoseInputs,
    scratch: &mut Scratch,
) -> Pose {
    let desired = desired_pose(inputs);
    if collides(view, inputs, Pose::Swimming, scratch) {
        return inputs.current;
    }
    if inputs.spectator || inputs.passenger || !collides(view, inputs, desired, scratch) {
        return desired;
    }
    if !collides(view, inputs, Pose::Crouching, scratch) {
        return Pose::Crouching;
    }
    Pose::Swimming
}

fn desired_pose(inputs: &PoseInputs) -> Pose {
    if inputs.sleeping {
        Pose::Sleeping
    } else if inputs.swimming {
        Pose::Swimming
    } else if inputs.fall_flying {
        Pose::FallFlying
    } else if inputs.spin_attack {
        Pose::SpinAttack
    } else if inputs.shift && !inputs.flight {
        Pose::Crouching
    } else {
        Pose::Standing
    }
}

fn collides<W: CollisionWorld, B: BodyView>(
    view: View<'_, W, B>,
    inputs: &PoseInputs,
    pose: Pose,
    scratch: &mut Scratch,
) -> bool {
    let dimensions = pose.player_dimensions().scaled(inputs.scale);
    pose_box_collides(view, &inputs.entity, dimensions, scratch)
}

pub(crate) fn pose_box_collides<W: CollisionWorld, B: BodyView>(
    view: View<'_, W, B>,
    entity: &QueryEntity,
    dimensions: Dimensions,
    scratch: &mut Scratch,
) -> bool {
    let bounds =
        Aabb::of_body(entity.position, dimensions.width, dimensions.height).deflate(COLLISION);
    query::intersects(view, entity, bounds, &mut scratch.entities)
}
