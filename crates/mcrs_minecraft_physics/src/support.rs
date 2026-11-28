use core::ops::ControlFlow;

use glam::DVec3;
use mcrs_voxel::BlockPos;

use crate::Aabb;
use crate::BlockFlags;
use crate::Collider;
use crate::CollisionTable;
use crate::CollisionWorld;
use crate::block_scan;
use crate::block_scan::ShapeContext;
use crate::block_scan::floor_block;
use crate::tolerance::CLIP;
use crate::tolerance::SUPPORT_DEPTH;

pub const SUPPORT_OFFSET: f32 = 1.0e-5;

pub const EFFECT_OFFSET: f32 = 0.2;

pub const MOVEMENT_OFFSET: f32 = 0.500_001;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Support {
    pub(crate) block: Option<BlockPos>,
    pub(crate) missed: bool,
}

pub(crate) fn find_support<W: CollisionWorld>(
    world: &W,
    table: &CollisionTable,
    context: &ShapeContext<'_>,
    bounds: &Aabb,
    resolved: DVec3,
    previous_missed: bool,
) -> Support {
    let area = Aabb::new(
        DVec3::new(bounds.min.x, bounds.min.y - SUPPORT_DEPTH, bounds.min.z),
        DVec3::new(bounds.max.x, bounds.min.y, bounds.max.z),
    );
    let position = context.entity.position;
    if let Some(block) = nearest_block(world, table, context, &area, position) {
        return Support {
            block: Some(block),
            missed: false,
        };
    }
    if previous_missed {
        return Support {
            block: None,
            missed: true,
        };
    }
    let shifted = area.translate(DVec3::new(-resolved.x, 0.0, -resolved.z));
    let block = nearest_block(world, table, context, &shifted, position);
    Support {
        block,
        missed: block.is_none(),
    }
}

pub fn offset_block(
    support: Option<BlockPos>,
    support_flags: BlockFlags,
    position: DVec3,
    offset: f32,
) -> BlockPos {
    let below = floor_block(position.y - f64::from(offset));
    let Some(support) = support else {
        return BlockPos::new(floor_block(position.x), below, floor_block(position.z));
    };
    if f64::from(offset) <= CLIP
        || support_flags.intersects(BlockFlags::WALL | BlockFlags::FENCE_GATE)
        || (support_flags.contains(BlockFlags::FENCE) && offset <= 0.5)
    {
        return support;
    }
    BlockPos::new(support.x, below, support.z)
}

fn nearest_block<W: CollisionWorld>(
    world: &W,
    table: &CollisionTable,
    context: &ShapeContext<'_>,
    area: &Aabb,
    position: DVec3,
) -> Option<BlockPos> {
    let mut best: Option<(f64, BlockPos)> = None;
    let _ = block_scan::scan(world, table, context, area, |collider, _| {
        if let Collider::Block { at, .. } = collider {
            let centre = at.as_dvec3() + DVec3::splat(0.5);
            let distance = centre.distance_squared(position);
            let better = match best {
                None => true,
                Some((nearest, held)) => {
                    distance < nearest || (distance == nearest && order_key(at) > order_key(held))
                }
            };
            if better {
                best = Some((distance, at));
            }
        }
        ControlFlow::Continue(())
    });
    best.map(|(_, at)| at)
}

fn order_key(at: BlockPos) -> (i32, i32, i32) {
    (at.y, at.z, at.x)
}
