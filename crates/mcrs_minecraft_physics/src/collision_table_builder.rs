use std::collections::HashMap;

use glam::DVec3;
use mcrs_voxel::BlockStateId;

use crate::Aabb;
use crate::BlockFlags;
use crate::BlockMotion;
use crate::CollisionTable;
use crate::InsideEffect;
use crate::Shape;
use crate::ShapeClass;
use crate::ShapeId;
use crate::StateCollision;

/// The input for one block state of a [`CollisionTableBuilder`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StateSpec<'a> {
    /// The shape for an entity that has no context, in block units relative to the block position.
    pub boxes: &'a [Aabb],
    /// The shape for an entity, when it differs from `boxes`.
    pub context: Option<ContextShape>,
    pub flags: BlockFlags,
    pub inside: InsideEffect,
    pub motion: BlockMotion,
}

/// A shape that depends on the entity.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ContextShape {
    Scaffolding { bottom: bool },
    PowderSnow,
}

/// Builds a [`CollisionTable`]. The states get their ids in the order of [`Self::push`].
#[derive(Debug)]
pub struct CollisionTableBuilder {
    states: Vec<StateCollision>,
    shapes: Vec<Shape>,
    interned: HashMap<Vec<u64>, ShapeId>,
    full_cube: ShapeId,
    scaffolding_top: ShapeId,
    scaffolding_bottom: ShapeId,
    powder_snow_deep: ShapeId,
}

const PIXEL: f64 = 1.0 / 16.0;

impl CollisionTableBuilder {
    #[must_use]
    pub fn new() -> Self {
        let mut builder = Self {
            states: Vec::new(),
            shapes: Vec::new(),
            interned: HashMap::new(),
            full_cube: ShapeId(0),
            scaffolding_top: ShapeId(0),
            scaffolding_bottom: ShapeId(0),
            powder_snow_deep: ShapeId(0),
        };
        builder.full_cube = builder
            .intern(&[unit(DVec3::ZERO, DVec3::ONE)])
            .unwrap_or(ShapeId(0));
        let post = 2.0 * PIXEL;
        let far = 14.0 * PIXEL;
        let scaffolding_top = [
            unit(DVec3::new(0.0, far, 0.0), DVec3::ONE),
            unit(DVec3::ZERO, DVec3::new(post, 1.0, post)),
            unit(DVec3::new(far, 0.0, 0.0), DVec3::new(1.0, 1.0, post)),
            unit(DVec3::new(0.0, 0.0, far), DVec3::new(post, 1.0, 1.0)),
            unit(DVec3::new(far, 0.0, far), DVec3::ONE),
        ];
        builder.scaffolding_top = builder.intern(&scaffolding_top).unwrap_or(ShapeId(0));
        builder.scaffolding_bottom = builder
            .intern(&[unit(DVec3::ZERO, DVec3::new(1.0, post, 1.0))])
            .unwrap_or(ShapeId(0));
        builder.powder_snow_deep = builder
            .intern(&[unit(DVec3::ZERO, DVec3::new(1.0, f64::from(0.9_f32), 1.0))])
            .unwrap_or(ShapeId(0));
        builder
    }

    /// Adds a state and returns its id, or `None` when the table is full.
    pub fn push(&mut self, spec: StateSpec<'_>) -> Option<BlockStateId> {
        let number = u16::try_from(self.states.len()).ok()?;
        let mut flags = spec.flags;
        if spec.boxes.iter().any(leaves_unit_cube) {
            flags |= BlockFlags::LARGE;
        }
        let class = match spec.context {
            Some(ContextShape::Scaffolding { bottom }) => ShapeClass::Scaffolding { bottom },
            Some(ContextShape::PowderSnow) => ShapeClass::PowderSnow,
            None => self.class_of(spec.boxes),
        };
        self.states.push(StateCollision {
            class,
            flags,
            inside: spec.inside,
            motion: spec.motion,
        });
        BlockStateId::checked(number, self.states.len())
    }

    #[must_use]
    pub fn build(self) -> CollisionTable {
        CollisionTable {
            states: self.states.into_boxed_slice(),
            shapes: self.shapes.into_boxed_slice(),
            full_cube: self.full_cube,
            scaffolding_top: self.scaffolding_top,
            scaffolding_bottom: self.scaffolding_bottom,
            powder_snow_deep: self.powder_snow_deep,
        }
    }

    fn class_of(&mut self, boxes: &[Aabb]) -> ShapeClass {
        let Some(id) = self.intern(boxes) else {
            return ShapeClass::Empty;
        };
        if id == self.full_cube {
            return ShapeClass::FullCube;
        }
        match self.shapes.get(id.0 as usize) {
            Some(shape) if shape.is_one_box() => ShapeClass::OneBox(id),
            Some(_) => ShapeClass::Compound(id),
            None => ShapeClass::Empty,
        }
    }

    fn intern(&mut self, boxes: &[Aabb]) -> Option<ShapeId> {
        let key: Vec<u64> = boxes
            .iter()
            .flat_map(|bounds| {
                bounds
                    .min
                    .to_array()
                    .into_iter()
                    .chain(bounds.max.to_array())
            })
            .map(f64::to_bits)
            .collect();
        if let Some(&id) = self.interned.get(&key) {
            return Some(id);
        }
        let shape = Shape::from_boxes(boxes)?;
        let id = ShapeId(u32::try_from(self.shapes.len()).ok()?);
        self.shapes.push(shape);
        self.interned.insert(key, id);
        Some(id)
    }
}

impl Default for CollisionTableBuilder {
    fn default() -> Self {
        Self::new()
    }
}

fn unit(min: DVec3, max: DVec3) -> Aabb {
    Aabb::new(min, max)
}

fn leaves_unit_cube(bounds: &Aabb) -> bool {
    bounds.min.min_element() < 0.0 || bounds.max.max_element() > 1.0
}
