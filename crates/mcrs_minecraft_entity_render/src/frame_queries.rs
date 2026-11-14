use mcrs_minecraft_entity::Pose;
use mcrs_minecraft_entity_animation::{EntityQuery, QuerySource};
use mcrs_minecraft_entity_visual::{FrameValues, TICKS_PER_SECOND};
use mcrs_molang::{Value, string_hash};

/// The variant of an entity, resolved by the host from its entity data, the default variant of
/// its type and the synced registry. An id that the registry does not hold has the hash of `''`
/// in each string.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VariantEntry {
    /// The numeric variant id: the position of the entry in the synced registry.
    pub id: f32,
    /// The [`string_hash`] of the resource location of the entry.
    pub identifier: u64,
    /// The [`string_hash`] of the asset identifier, the baby one for a baby.
    pub asset: u64,
    /// The [`string_hash`] of the model type in lower case.
    pub model: u64,
}

/// The queries of one client entity in one frame, read from [`FrameValues`].
#[derive(Clone, Copy, Debug)]
pub struct FrameQueries<'a> {
    pub frame: &'a FrameValues,
    /// `None` for a type with no variant.
    pub variant: Option<VariantEntry>,
    pub time_of_day: f32,
    /// An in-water flag of the host that adds to [`FrameValues::in_water`].
    pub in_water: bool,
}

impl FrameQueries<'_> {
    fn flag(value: bool) -> Value {
        Value::Number(if value { 1.0 } else { 0.0 })
    }
}

impl QuerySource for FrameQueries<'_> {
    fn query(&self, query: EntityQuery) -> Option<Value> {
        let frame = self.frame;
        let empty = Value::String(string_hash(""));
        let value = match query {
            EntityQuery::LifeTime => Value::Number(frame.clock / TICKS_PER_SECOND),
            EntityQuery::FrameAlpha => Value::Number(frame.partial_tick),
            EntityQuery::TimeOfDay => Value::Number(self.time_of_day),
            EntityQuery::ModifiedDistanceMoved => Value::Number(frame.walk.position),
            EntityQuery::ModifiedMoveSpeed => Value::Number(frame.walk.speed),
            EntityQuery::GroundSpeed => Value::Number(frame.ground_speed as f32),
            EntityQuery::VerticalSpeed => Value::Number(frame.vertical_speed as f32),
            EntityQuery::IsMoving => Self::flag(frame.moving),
            EntityQuery::IsOnGround => Self::flag(frame.on_ground),
            EntityQuery::IsInWater => Self::flag(frame.in_water || self.in_water),
            EntityQuery::IsRiding => Self::flag(frame.riding),
            EntityQuery::HasRider => Self::flag(frame.has_rider),
            EntityQuery::TargetXRotation | EntityQuery::HeadXRotation(Some(0)) => {
                Value::Number(frame.pitch)
            }
            EntityQuery::TargetYRotation => Value::Number(frame.relative_head_yaw),
            EntityQuery::BodyYRotation => Value::Number(frame.body_yaw),
            EntityQuery::HeadYRotation(Some(0)) => Value::Number(frame.head_yaw),
            EntityQuery::IsBaby => Self::flag(frame.baby),
            EntityQuery::IsAlive => Self::flag(frame.alive),
            EntityQuery::HurtTime => Value::Number(frame.hurt_time as f32),
            EntityQuery::DeathTicks => Value::Number(frame.death_ticks),
            EntityQuery::Variant => Value::Number(self.variant.map_or(0.0, |entry| entry.id)),
            EntityQuery::VariantIdentifier => self
                .variant
                .map_or(empty, |entry| Value::String(entry.identifier)),
            EntityQuery::VariantAsset => self
                .variant
                .map_or(empty, |entry| Value::String(entry.asset)),
            EntityQuery::VariantModel => self
                .variant
                .map_or(empty, |entry| Value::String(entry.model)),
            EntityQuery::ModelScale => Value::Number(frame.model_scale as f32),
            EntityQuery::IsSaddled => Self::flag(frame.saddled),
            EntityQuery::Pose => Value::String(string_hash(pose_name(frame.pose))),
            EntityQuery::IsOnFire => Self::flag(frame.on_fire),
            EntityQuery::IsSneaking => Self::flag(frame.sneaking),
            EntityQuery::IsSprinting => Self::flag(frame.sprinting),
            EntityQuery::IsSwimming => Self::flag(frame.swimming),
            EntityQuery::IsInvisible => Self::flag(frame.invisible),
            _ => Value::Number(0.0),
        };
        Some(value)
    }
}

const POSES: [(Pose, &str); 18] = [
    (Pose::Standing, "standing"),
    (Pose::FallFlying, "fall_flying"),
    (Pose::Sleeping, "sleeping"),
    (Pose::Swimming, "swimming"),
    (Pose::SpinAttack, "spin_attack"),
    (Pose::Crouching, "crouching"),
    (Pose::LongJumping, "long_jumping"),
    (Pose::Dying, "dying"),
    (Pose::Croaking, "croaking"),
    (Pose::UsingTongue, "using_tongue"),
    (Pose::Sitting, "sitting"),
    (Pose::Roaring, "roaring"),
    (Pose::Sniffing, "sniffing"),
    (Pose::Emerging, "emerging"),
    (Pose::Digging, "digging"),
    (Pose::Sliding, "sliding"),
    (Pose::Shooting, "shooting"),
    (Pose::Inhaling, "inhaling"),
];

/// The value of `query.pose` for a pose of the entity data.
#[must_use]
pub fn pose_name(pose: Pose) -> &'static str {
    POSES
        .iter()
        .find(|(candidate, _)| *candidate == pose)
        .map_or("standing", |(_, name)| name)
}

/// The pose that `query.pose` names, or `None` for a name that no pose has.
#[must_use]
pub fn pose_from_name(name: &str) -> Option<Pose> {
    POSES
        .iter()
        .find(|(_, candidate)| *candidate == name)
        .map(|(pose, _)| *pose)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_pose_name_names_its_pose_back() {
        for (pose, name) in POSES {
            assert_eq!(pose_name(pose), name);
            assert_eq!(pose_from_name(name), Some(pose));
        }
        assert_eq!(pose_from_name("floating"), None);
    }
}
