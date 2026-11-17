use glam::{Affine3A, Vec3};

use crate::baked_geometry::{BakedBone, BakedGeometry};
use crate::space;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BonePose {
    pub position: Vec3,
    pub rotation: Vec3,
    pub scale: Vec3,
}

impl BonePose {
    #[must_use]
    pub fn rest(bone: &BakedBone) -> Self {
        Self {
            position: Vec3::ZERO,
            rotation: bone.rotation,
            scale: Vec3::ONE,
        }
    }

    #[must_use]
    pub fn local_transform(&self, pivot: Vec3) -> Affine3A {
        Affine3A::from_translation(self.position)
            * space::about(
                pivot,
                Affine3A::from_mat3(space::euler(self.rotation)) * Affine3A::from_scale(self.scale),
            )
    }
}

impl BakedGeometry {
    #[must_use]
    pub fn model_transforms(&self, poses: &[BonePose]) -> Vec<Affine3A> {
        let mut transforms: Vec<Affine3A> = Vec::with_capacity(self.bones.len());
        for (index, bone) in self.bones.iter().enumerate() {
            let pose = poses
                .get(index)
                .copied()
                .unwrap_or_else(|| BonePose::rest(bone));
            let local = pose.local_transform(bone.pivot);
            let parent = bone
                .parent
                .and_then(|parent| transforms.get(parent))
                .copied()
                .unwrap_or(Affine3A::IDENTITY);
            transforms.push(parent * local);
        }
        transforms
    }
}
