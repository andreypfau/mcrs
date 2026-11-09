use mcrs_voxel::BlockStateId;

use crate::Aabb;
use crate::BlockFlags;
use crate::BlockMotion;
use crate::BoxRange;
use crate::InsideEffect;
use crate::InsideShape;
use crate::PathType;
use crate::Shape;

/// The index of a shape in a [`CollisionTable`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ShapeId(pub(crate) u32);

/// How the collision shape of a block state is found.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShapeClass {
    Empty,
    FullCube,
    OneBox(ShapeId),
    Compound(ShapeId),
    /// The shape depends on the bottom of the entity and its shift key. `bottom` is true when the
    /// state can give the bottom slab: a distance that is not 0 and the bottom property.
    Scaffolding {
        bottom: bool,
    },
    /// The shape depends on the fall distance and the type of the entity.
    PowderSnow,
}

/// The collision data of one block state.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StateCollision {
    pub class: ShapeClass,
    pub flags: BlockFlags,
    pub inside: InsideEffect,
    pub inside_shape: InsideShape,
    pub motion: BlockMotion,
    /// The state path type of PATH-TYP-002.
    pub path: PathType,
}

impl StateCollision {
    pub const AIR: StateCollision = StateCollision {
        class: ShapeClass::Empty,
        flags: BlockFlags::empty(),
        inside: InsideEffect::None,
        inside_shape: InsideShape::FullCube,
        motion: BlockMotion::DEFAULT,
        path: PathType::Open,
    };

    /// The state acts on an entity whose box is inside it: an inside effect, an inside action or a
    /// fall reset.
    #[must_use]
    pub fn acts_inside(&self) -> bool {
        self.inside != InsideEffect::None
            || self
                .flags
                .intersects(BlockFlags::INSIDE_ACTION | BlockFlags::FALL_DAMAGE_RESETTING)
    }

    /// The state has a collision shape, a context shape, or a moving piston: it sets its bit in
    /// the `solid` mask of a section.
    #[must_use]
    pub fn is_solid(&self) -> bool {
        self.class != ShapeClass::Empty || self.flags.contains(BlockFlags::MOVING_PISTON)
    }
}

/// The collision shapes, flags and motion data of every block state.
#[derive(Clone, Debug, PartialEq)]
pub struct CollisionTable {
    pub(crate) states: Vec<StateCollision>,
    pub(crate) shapes: Vec<Shape>,
    pub(crate) inside_boxes: Vec<Aabb>,
    /// The inside shape of powder snow for an entity that falls more than 2.5 blocks.
    pub(crate) powder_snow_deep_inside: BoxRange,
    pub(crate) full_cube: ShapeId,
    pub(crate) scaffolding_top: ShapeId,
    /// The bottom slab of a scaffolding, from `y = 0` to `2/16`.
    pub(crate) scaffolding_bottom: ShapeId,
    /// The powder snow shape for an entity that falls more than 2.5 blocks.
    pub(crate) powder_snow_deep: ShapeId,
}

impl CollisionTable {
    /// The data of a state; a state out of the table is air.
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

    /// The boxes of an inside shape, relative to the block position.
    #[must_use]
    pub fn inside_boxes(&self, range: BoxRange) -> &[Aabb] {
        self.inside_boxes
            .get(range.start as usize..range.end as usize)
            .unwrap_or(&[])
    }

    /// The shape of `class` for no entity: the shape of the block definition. A scaffolding has
    /// its top shape, and a powder snow block has none.
    #[must_use]
    pub fn shape_without_context(&self, class: ShapeClass) -> Option<&Shape> {
        match class {
            ShapeClass::Empty | ShapeClass::PowderSnow => None,
            ShapeClass::FullCube => self.shape(self.full_cube),
            ShapeClass::OneBox(id) | ShapeClass::Compound(id) => self.shape(id),
            ShapeClass::Scaffolding { .. } => self.shape(self.scaffolding_top),
        }
    }

    /// The scaffolding shape for an entity above it: the top slab and the four corner posts.
    #[must_use]
    pub fn scaffolding_top(&self) -> ShapeId {
        self.scaffolding_top
    }
}
