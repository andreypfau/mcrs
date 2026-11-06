use core::ops::ControlFlow;

use mcrs_voxel::BlockPos;
use mcrs_voxel::BlockStateId;
use mcrs_voxel::LocalPos;
use mcrs_voxel::SectionPos;

use crate::Aabb;
use crate::BlockFlags;
use crate::Collider;
use crate::CollisionSection;
use crate::CollisionTable;
use crate::CollisionWorld;
use crate::QueryEntity;
use crate::QueryVariant;
use crate::SectionLookup;
use crate::ShapeClass;
use crate::ShapeId;
use crate::StateCollision;
use crate::tolerance::CLIP;
use crate::tolerance::COLLISION;

/// The entity values that select the shape of a block.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct ShapeContext<'a> {
    pub(crate) entity: &'a QueryEntity,
    pub(crate) feet: f64,
    pub(crate) variant: QueryVariant,
}

/// Visits the block colliders of `query` in the scan order: `x` fastest, then `y`, then `z`,
/// over the region one block larger than the query. A position on an outer face of the region
/// counts only with a large shape, one on an outer edge only for a moving piston, and an outer
/// corner never.
pub(crate) fn scan<W: CollisionWorld>(
    world: &W,
    table: &CollisionTable,
    context: &ShapeContext<'_>,
    query: &Aabb,
    mut visit: impl FnMut(Collider, BlockStateId) -> ControlFlow<()>,
) -> ControlFlow<()> {
    let [x0, y0, z0] = query
        .min
        .to_array()
        .map(|low| floor_block(low - COLLISION).saturating_sub(1));
    let [x1, y1, z1] = query
        .max
        .to_array()
        .map(|high| floor_block(high + COLLISION).saturating_add(1));
    let mut cache = SectionCache::default();
    for z in z0..=z1 {
        let z_face = u8::from(z == z0 || z == z1);
        for y in y0..=y1 {
            let yz_faces = z_face + u8::from(y == y0 || y == y1);
            let mut x = x0;
            while x <= x1 {
                let run_end = (x | 15).min(x1);
                if let Some(section) = cache.get(world, SectionPos::new(x >> 4, y >> 4, z >> 4)) {
                    let run = Run {
                        y,
                        z,
                        xs: [x, run_end],
                        ends: [x0, x1],
                        yz_faces,
                    };
                    scan_run(table, context, query, section, &run, &mut visit)?;
                }
                let Some(next) = run_end.checked_add(1) else {
                    break;
                };
                x = next;
            }
        }
    }
    ControlFlow::Continue(())
}

/// The blocks of one row of the scan inside one section.
struct Run {
    y: i32,
    z: i32,
    xs: [i32; 2],
    ends: [i32; 2],
    yz_faces: u8,
}

fn scan_run<S: CollisionSection>(
    table: &CollisionTable,
    context: &ShapeContext<'_>,
    query: &Aabb,
    section: &S,
    run: &Run,
    visit: &mut impl FnMut(Collider, BlockStateId) -> ControlFlow<()>,
) -> ControlFlow<()> {
    let [first, last] = run.xs;
    let [x0, x1] = run.ends;
    if let Some(masks) = section.masks() {
        let row = ((run.z >> 2) & 3) << 2 | ((run.y >> 2) & 3) << 4;
        let bricks = (0b1111_u64 << ((first >> 2) & 3)) & (0b1111_u64 >> (3 - ((last >> 2) & 3)));
        if masks.solid >> row & bricks == 0 {
            return ControlFlow::Continue(());
        }
    }
    for x in first..=last {
        let faces = run.yz_faces + u8::from(x == x0 || x == x1);
        let at = BlockPos::new(x, run.y, run.z);
        if let Some((collider, state)) = collider_at(table, context, query, section, at, faces) {
            visit(collider, state)?;
        }
    }
    ControlFlow::Continue(())
}

fn collider_at<S: CollisionSection>(
    table: &CollisionTable,
    context: &ShapeContext<'_>,
    query: &Aabb,
    section: &S,
    at: BlockPos,
    faces: u8,
) -> Option<(Collider, BlockStateId)> {
    if faces >= 3 {
        return None;
    }
    let local = LocalPos::from(at);
    if let Some(masks) = section.masks()
        && !masks.may_collide(local)
    {
        return None;
    }
    let state = section.state(local);
    let data = table.state(state);
    let counts = match faces {
        0 => true,
        1 => data.flags.contains(BlockFlags::LARGE),
        _ => data.flags.contains(BlockFlags::MOVING_PISTON),
    };
    if !counts {
        return None;
    }
    let shape = entity_shape(table, data, at, context)?;
    let collider = Collider::Block { shape, at };
    collider
        .intersects(table, query)
        .then_some((collider, state))
}

/// The shape of a state for the entity of the query.
pub(crate) fn entity_shape(
    table: &CollisionTable,
    data: StateCollision,
    at: BlockPos,
    context: &ShapeContext<'_>,
) -> Option<ShapeId> {
    let entity = context.entity;
    let feet = context.feet;
    let placement = context.variant == QueryVariant::Placement;
    let above = feet > f64::from(at.y) + 1.0 - CLIP;
    match data.class {
        ShapeClass::Empty => None,
        ShapeClass::FullCube => Some(table.full_cube()),
        ShapeClass::OneBox(shape) | ShapeClass::Compound(shape) => Some(shape),
        ShapeClass::Scaffolding { .. } if placement => None,
        ShapeClass::Scaffolding { bottom } => {
            if above && !entity.shift {
                Some(table.scaffolding_top())
            } else if bottom && feet > f64::from(at.y) - CLIP {
                Some(table.scaffolding_bottom())
            } else {
                None
            }
        }
        ShapeClass::PowderSnow if placement => None,
        ShapeClass::PowderSnow => {
            if entity.fall_distance > 2.5 {
                Some(table.powder_snow_deep())
            } else if entity.powder_snow_solid
                || (entity.walks_on_powder_snow && above && !entity.shift)
            {
                Some(table.full_cube())
            } else {
                None
            }
        }
    }
}

/// The block that holds a coordinate, saturated to the range of `i32`.
pub(crate) fn floor_block(value: f64) -> i32 {
    value.floor() as i32
}

/// The block state at a position; a block out of the world or in a chunk that is not loaded is
/// air.
pub(crate) fn state_at<W: CollisionWorld>(world: &W, at: BlockPos) -> BlockStateId {
    match world.section(SectionPos::from(at)) {
        SectionLookup::Loaded(section) => section.state(LocalPos::from(at)),
        SectionLookup::Outside | SectionLookup::Unloaded => BlockStateId::AIR,
    }
}

/// The last section of the scan. The scan visits the sections of one row in turn, and then the
/// same sections again for the next row.
struct SectionCache<S> {
    entries: [Option<(SectionPos, SectionLookup<S>)>; 2],
    next: usize,
}

impl<S> Default for SectionCache<S> {
    fn default() -> Self {
        Self {
            entries: [None, None],
            next: 0,
        }
    }
}

impl<S> SectionCache<S> {
    fn get<'w, W>(&mut self, world: &'w W, at: SectionPos) -> Option<&S>
    where
        W: CollisionWorld<Section<'w> = S>,
    {
        let hit = self
            .entries
            .iter()
            .position(|entry| entry.as_ref().is_some_and(|(pos, _)| *pos == at));
        let slot = match hit {
            Some(slot) => slot,
            None => {
                let slot = self.next;
                self.next = (self.next + 1) % self.entries.len();
                if let Some(entry) = self.entries.get_mut(slot) {
                    *entry = Some((at, world.section(at)));
                }
                slot
            }
        };
        match self.entries.get(slot) {
            Some(Some((_, SectionLookup::Loaded(section)))) => Some(section),
            _ => None,
        }
    }
}
