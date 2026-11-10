use glam::Vec3;
use mcrs_molang::ScalarVm;

use crate::channel::Channel;
use crate::clip::{Clip, ClipBone};
use crate::entity_animation::EntityAnimation;
use crate::pose_binding::PoseBinding;
use crate::rig::{AnimateTarget, Rig};

/// The pose of one bone of a rig in file space.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PoseBone {
    /// The rotation in degrees that the clips add to the rest rotation.
    pub rotation: Vec3,
    pub position: Vec3,
    pub scale: Vec3,
    /// A clip sets `relative_to.rotation` to `"entity"` for the bone in this frame.
    pub relative_to_entity: bool,
}

impl PoseBone {
    pub const REST: Self = Self {
        rotation: Vec3::ZERO,
        position: Vec3::ZERO,
        scale: Vec3::ONE,
        relative_to_entity: false,
    };
}

/// The pose of each bone of a rig in one frame, in the order of [`Rig::bones`]. Each geometry
/// of the entity takes the pose by bone name.
#[derive(Clone, Debug, PartialEq)]
pub struct Pose {
    pub bones: Vec<PoseBone>,
}

impl Pose {
    #[must_use]
    pub fn new(rig: &Rig) -> Self {
        Self {
            bones: vec![PoseBone::REST; rig.bones.len()],
        }
    }
}

/// A channel of a pose bone.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ChannelKind {
    Rotation,
    Position,
    Scale,
}

impl ChannelKind {
    /// The value of an axis that `this` reads: the pose before the instance applies it.
    pub fn current(self, bone: &PoseBone, rest: Vec3, axis: usize) -> f32 {
        let value = match self {
            Self::Rotation => rest + bone.rotation,
            Self::Position => bone.position,
            Self::Scale => bone.scale,
        };
        value.to_array().get(axis).copied().unwrap_or(0.0)
    }

    /// Adds a sample with the weight `weight`. An axis that is not finite changes nothing.
    pub fn apply(self, bone: &mut PoseBone, sample: [f32; 3], weight: f32) {
        let target = match self {
            Self::Rotation => &mut bone.rotation,
            Self::Position => &mut bone.position,
            Self::Scale => &mut bone.scale,
        };
        let mut values = target.to_array();
        for (current, value) in values.iter_mut().zip(sample) {
            if !value.is_finite() {
                continue;
            }
            *current = match self {
                Self::Scale => *current * (1.0 + (value - 1.0) * weight),
                Self::Rotation | Self::Position => *current + weight * value,
            };
        }
        *target = Vec3::from_array(values);
    }
}

impl ClipBone {
    pub(crate) fn channels(&self) -> [(ChannelKind, Option<&Channel>); 3] {
        [
            (ChannelKind::Rotation, self.rotation.as_ref()),
            (ChannelKind::Position, self.position.as_ref()),
            (ChannelKind::Scale, self.scale.as_ref()),
        ]
    }

    /// Prepares the bone before `clip` applies its channels: an override resets it, and a
    /// rotation relative to the entity marks it.
    pub(crate) fn begin(&self, clip: &Clip, bone: &mut PoseBone) {
        if clip.override_previous {
            bone.rotation = Vec3::ZERO;
            bone.position = Vec3::ZERO;
            if self.scale.is_some() {
                bone.scale = Vec3::ONE;
            }
        }
        bone.relative_to_entity |= self.relative_to_entity;
    }
}

impl EntityAnimation {
    /// Samples the channels of the play list of this frame into `pose`, after
    /// [`EntityAnimation::update`]. `binding` gives the rest rotations that `this` and an
    /// override read.
    pub fn pose(&mut self, rig: &Rig, vm: &mut ScalarVm, binding: &PoseBinding, pose: &mut Pose) {
        pose.bones.clear();
        pose.bones.resize(rig.bones.len(), PoseBone::REST);
        for (index, entry) in rig.animate.iter().enumerate() {
            let AnimateTarget::Clip(clip) = entry.target;
            let (Some(instance), Some(clip)) =
                (self.instances.get(index).copied(), rig.clips.get(clip))
            else {
                continue;
            };
            if instance.weight == 0.0 {
                continue;
            }
            let Some(time) = instance.sample_time(clip) else {
                continue;
            };
            self.set_number(rig.queries.anim_time, instance.time);
            self.apply_clip(rig, vm, clip, time, instance.weight, binding, pose);
        }
        self.set_number(rig.queries.anim_time, 0.0);
        self.set_number(rig.queries.key_frame_lerp_time, 0.0);
    }

    #[expect(clippy::too_many_arguments, reason = "one clip of one frame")]
    fn apply_clip(
        &mut self,
        rig: &Rig,
        vm: &mut ScalarVm,
        clip: &Clip,
        time: f32,
        weight: f32,
        binding: &PoseBinding,
        pose: &mut Pose,
    ) {
        for clip_bone in &clip.bones {
            let Some(bone) = pose.bones.get_mut(clip_bone.bone) else {
                continue;
            };
            let rest = binding.rest_rotation(clip_bone.bone);
            clip_bone.begin(clip, bone);
            for (kind, channel) in clip_bone.channels() {
                let Some(channel) = channel else {
                    continue;
                };
                let Some(segment) = channel.segment(time) else {
                    continue;
                };
                self.set_number(rig.queries.key_frame_lerp_time, segment.alpha());
                let before = *bone;
                let sample = channel.sample(segment, |expression, axis| {
                    self.number(vm, expression, kind.current(&before, rest, axis))
                });
                kind.apply(bone, sample, weight);
            }
        }
    }
}
