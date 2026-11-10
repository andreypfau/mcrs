use std::collections::HashMap;

use glam::DVec3;
use mcrs_voxel::BlockStateId;

use crate::Aabb;
use crate::BlockFlags;
use crate::BlockMotion;
use crate::BoxRange;
use crate::CollisionTable;
use crate::InsideBoxes;
use crate::InsideEffect;
use crate::InsideShape;
use crate::PathType;
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
    pub inside_shape: InsideBoxes<'a>,
    pub motion: BlockMotion,
    pub path: PathType,
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
    table: CollisionTable,
    interned: HashMap<Vec<u64>, ShapeId>,
}

const PIXEL: f64 = 1.0 / 16.0;

impl CollisionTableBuilder {
    #[must_use]
    pub fn new() -> Self {
        let mut builder = Self {
            table: CollisionTable {
                states: Vec::new(),
                shapes: Vec::new(),
                inside_boxes: Vec::new(),
                powder_snow_deep_inside: BoxRange { start: 0, end: 0 },
                full_cube: ShapeId(0),
                scaffolding_top: ShapeId(0),
                scaffolding_bottom: ShapeId(0),
                powder_snow_deep: ShapeId(0),
            },
            interned: HashMap::new(),
        };
        builder.table.full_cube = builder
            .intern(&[Aabb::new(DVec3::ZERO, DVec3::ONE)])
            .unwrap_or(ShapeId(0));
        let post = 2.0 * PIXEL;
        let far = 14.0 * PIXEL;
        builder.table.scaffolding_top = builder
            .intern(&[
                Aabb::new(DVec3::new(0.0, far, 0.0), DVec3::ONE),
                Aabb::new(DVec3::ZERO, DVec3::new(post, 1.0, post)),
                Aabb::new(DVec3::new(far, 0.0, 0.0), DVec3::new(1.0, 1.0, post)),
                Aabb::new(DVec3::new(0.0, 0.0, far), DVec3::new(post, 1.0, 1.0)),
                Aabb::new(DVec3::new(far, 0.0, far), DVec3::ONE),
            ])
            .unwrap_or(ShapeId(0));
        builder.table.scaffolding_bottom = builder
            .intern(&[Aabb::new(DVec3::ZERO, DVec3::new(1.0, post, 1.0))])
            .unwrap_or(ShapeId(0));
        let powder_snow_deep = [Aabb::new(
            DVec3::ZERO,
            DVec3::new(1.0, f64::from(0.9_f32), 1.0),
        )];
        builder.table.powder_snow_deep = builder.intern(&powder_snow_deep).unwrap_or(ShapeId(0));
        builder.table.powder_snow_deep_inside = builder
            .push_inside_boxes(&powder_snow_deep)
            .unwrap_or(BoxRange { start: 0, end: 0 });
        builder
    }

    /// Adds a state and returns its id, or `None` when the table is full.
    pub fn push(&mut self, spec: StateSpec<'_>) -> Option<BlockStateId> {
        let number = u16::try_from(self.table.states.len()).ok()?;
        let mut flags = spec.flags;
        if spec.boxes.iter().any(leaves_unit_cube) {
            flags |= BlockFlags::LARGE;
        }
        let class = match spec.context {
            Some(ContextShape::Scaffolding { bottom }) => ShapeClass::Scaffolding { bottom },
            Some(ContextShape::PowderSnow) => ShapeClass::PowderSnow,
            None => self.class_of(spec.boxes),
        };
        let inside_shape = match spec.inside_shape {
            InsideBoxes::FullCube => InsideShape::FullCube,
            InsideBoxes::Boxes(boxes) => InsideShape::Boxes(self.push_inside_boxes(boxes)?),
            InsideBoxes::Collision => InsideShape::Collision,
        };
        self.table.states.push(StateCollision {
            class,
            flags,
            inside: spec.inside,
            inside_shape,
            motion: spec.motion,
            path: spec.path,
        });
        BlockStateId::checked(number, self.table.states.len())
    }

    #[must_use]
    pub fn build(self) -> CollisionTable {
        self.table
    }

    fn push_inside_boxes(&mut self, boxes: &[Aabb]) -> Option<BoxRange> {
        let inside_boxes = &mut self.table.inside_boxes;
        let start = u32::try_from(inside_boxes.len()).ok()?;
        inside_boxes.extend_from_slice(boxes);
        let end = u32::try_from(inside_boxes.len()).ok()?;
        Some(BoxRange { start, end })
    }

    fn class_of(&mut self, boxes: &[Aabb]) -> ShapeClass {
        let Some(id) = self.intern(boxes) else {
            return ShapeClass::Empty;
        };
        if id == self.table.full_cube {
            return ShapeClass::FullCube;
        }
        match self.table.shape(id) {
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
        let shapes = &mut self.table.shapes;
        let id = ShapeId(u32::try_from(shapes.len()).ok()?);
        shapes.push(shape);
        self.interned.insert(key, id);
        Some(id)
    }
}

impl Default for CollisionTableBuilder {
    fn default() -> Self {
        Self::new()
    }
}

fn leaves_unit_cube(bounds: &Aabb) -> bool {
    bounds.min.min_element() < 0.0 || bounds.max.max_element() > 1.0
}
