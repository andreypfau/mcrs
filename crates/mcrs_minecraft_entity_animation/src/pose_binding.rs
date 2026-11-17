use glam::{Affine3A, Vec3};
use mcrs_minecraft_entity_model::{BakedGeometry, BonePose, space};

use crate::pose::{Pose, PoseBone};
use crate::rig::Rig;

#[derive(Clone, Debug, PartialEq)]
pub struct PoseBinding {
    rest: Vec<Vec3>,
    rig_bones: Vec<Option<usize>>,
}

impl PoseBinding {
    #[must_use]
    pub fn new(rig: &Rig, geometry: &BakedGeometry) -> Self {
        let rig_bones: Vec<Option<usize>> = geometry
            .bones
            .iter()
            .map(|bone| rig.bone(&bone.name))
            .collect();
        let mut rest = vec![Vec3::ZERO; rig.bones.len()];
        for (bone, rig_bone) in geometry.bones.iter().zip(&rig_bones) {
            if let Some(held) = rig_bone.and_then(|index| rest.get_mut(index)) {
                *held = bone.file_rotation;
            }
        }
        Self { rest, rig_bones }
    }

    #[must_use]
    pub fn rest_rotation(&self, rig_bone: usize) -> Vec3 {
        self.rest.get(rig_bone).copied().unwrap_or(Vec3::ZERO)
    }

    pub fn bone_poses(&self, geometry: &BakedGeometry, pose: &Pose, poses: &mut Vec<BonePose>) {
        poses.clear();
        for (bone, rig_bone) in geometry.bones.iter().zip(&self.rig_bones) {
            let Some(animated) = rig_bone.and_then(|index| pose.bones.get(index)) else {
                poses.push(BonePose::rest(bone));
                continue;
            };
            poses.push(BonePose {
                position: space::internal_point(animated.position),
                rotation: space::internal_rotation(bone.file_rotation + animated.rotation),
                scale: animated.scale,
            });
        }
    }

    #[must_use]
    pub fn model_transforms(&self, geometry: &BakedGeometry, pose: &Pose) -> Vec<Affine3A> {
        let mut poses = Vec::with_capacity(geometry.bones.len());
        self.bone_poses(geometry, pose, &mut poses);
        let mut transforms: Vec<Affine3A> = Vec::with_capacity(geometry.bones.len());
        for ((bone, bone_pose), rig_bone) in geometry.bones.iter().zip(&poses).zip(&self.rig_bones)
        {
            let parent = bone
                .parent
                .and_then(|parent| transforms.get(parent))
                .copied()
                .unwrap_or(Affine3A::IDENTITY);
            let relative = rig_bone
                .and_then(|index| pose.bones.get(index))
                .is_some_and(|animated: &PoseBone| animated.relative_to_entity);
            let transform = if relative {
                let pivot = parent.transform_point3(bone_pose.position + bone.pivot);
                let turn = Affine3A::from_mat3(space::euler(bone_pose.rotation))
                    * Affine3A::from_scale(bone_pose.scale);
                Affine3A::from_translation(pivot) * turn * Affine3A::from_translation(-bone.pivot)
            } else {
                parent * bone_pose.local_transform(bone.pivot)
            };
            transforms.push(transform);
        }
        transforms
    }
}
