use mcrs_voxel::BlockStateId;

use crate::Aabb;
use crate::BlockFlags;
use crate::BlockMotion;
use crate::BoxRange;
use crate::InsideEffect;
use crate::InsideShape;
use crate::PathType;
use crate::Shape;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ShapeId(pub(crate) u32);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShapeClass {
    Empty,
    FullCube,
    OneBox(ShapeId),
    Compound(ShapeId),
    Scaffolding { bottom: bool },
    PowderSnow,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StateCollision {
    pub class: ShapeClass,
    pub flags: BlockFlags,
    pub inside: InsideEffect,
    pub inside_shape: InsideShape,
    pub motion: BlockMotion,
    pub path: PathType,
}

const _: () = assert!(size_of::<StateCollision>() == 44);

impl StateCollision {
    pub const AIR: StateCollision = StateCollision {
        class: ShapeClass::Empty,
        flags: BlockFlags::empty(),
        inside: InsideEffect::None,
        inside_shape: InsideShape::FullCube,
        motion: BlockMotion::DEFAULT,
        path: PathType::Open,
    };

    #[must_use]
    pub fn acts_inside(&self) -> bool {
        self.inside != InsideEffect::None
            || self
                .flags
                .intersects(BlockFlags::INSIDE_ACTION | BlockFlags::FALL_DAMAGE_RESETTING)
    }

    #[must_use]
    pub fn is_solid(&self) -> bool {
        self.class != ShapeClass::Empty || self.flags.contains(BlockFlags::MOVING_PISTON)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct CollisionTable {
    pub(crate) states: Vec<StateCollision>,
    pub(crate) shapes: Vec<Shape>,
    pub(crate) inside_boxes: Vec<Aabb>,
    pub(crate) powder_snow_deep_inside: BoxRange,
    pub(crate) full_cube: ShapeId,
    pub(crate) scaffolding_top: ShapeId,
    pub(crate) scaffolding_bottom: ShapeId,
    pub(crate) powder_snow_deep: ShapeId,
}

impl CollisionTable {
    #[must_use]
    pub fn state(&self, id: BlockStateId) -> StateCollision {
        self.states
            .get(id.index())
            .copied()
            .unwrap_or(StateCollision::AIR)
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.states.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.states.is_empty()
    }

    #[must_use]
    pub fn shape(&self, id: ShapeId) -> Option<&Shape> {
        self.shapes.get(id.0 as usize)
    }

    #[must_use]
    pub fn full_cube(&self) -> ShapeId {
        self.full_cube
    }

    #[must_use]
    pub fn inside_boxes(&self, range: BoxRange) -> &[Aabb] {
        self.inside_boxes
            .get(range.start as usize..range.end as usize)
            .unwrap_or(&[])
    }

    #[must_use]
    pub fn shape_without_context(&self, class: ShapeClass) -> Option<&Shape> {
        match class {
            ShapeClass::Empty | ShapeClass::PowderSnow => None,
            ShapeClass::FullCube => self.shape(self.full_cube),
            ShapeClass::OneBox(id) | ShapeClass::Compound(id) => self.shape(id),
            ShapeClass::Scaffolding { .. } => self.shape(self.scaffolding_top),
        }
    }

    #[must_use]
    pub fn scaffolding_top(&self) -> ShapeId {
        self.scaffolding_top
    }
}
