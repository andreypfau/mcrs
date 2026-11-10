use glam::DVec3;
use glam::IVec3;
use mcrs_voxel::BlockPos;
use mcrs_voxel::LocalPos;
use mcrs_voxel::SectionPos;

use crate::CollisionSection;
use crate::CollisionTable;
use crate::CollisionWorld;
use crate::SectionLookup;
use crate::ShapeClass;

/// True when the segment from `from` to `to` meets no block shape for no context
/// ([`CollisionTable::shape_without_context`]); fluids do not stop it. A block out of the world is
/// air, and a block in a chunk that is not loaded is a full cube.
///
/// The ray visits each block that the segment crosses, so the caller bounds its length.
#[must_use]
pub fn sight_clear<W: CollisionWorld>(
    world: &W,
    table: &CollisionTable,
    from: DVec3,
    to: DVec3,
) -> bool {
    let start = from;
    let delta = to - start;
    let mut cell = start.floor().as_ivec3();
    let last = (start + delta).floor().as_ivec3();
    let step = IVec3::new(sign(delta.x), sign(delta.y), sign(delta.z));
    let inverse = delta.abs().recip();
    let first_edge = |position: f64, delta: f64, inverse: f64| {
        if delta == 0.0 {
            f64::INFINITY
        } else if delta > 0.0 {
            (position.floor() + 1.0 - position) * inverse
        } else {
            (position - position.floor()) * inverse
        }
    };
    let mut next = DVec3::new(
        first_edge(start.x, delta.x, inverse.x),
        first_edge(start.y, delta.y, inverse.y),
        first_edge(start.z, delta.z, inverse.z),
    );
    let mut section: Option<(SectionPos, SectionLookup<W::Section<'_>>)> = None;
    let blocks_crossed = (last - cell).abs().element_sum();
    for _ in 0..=blocks_crossed {
        let at = BlockPos::new(cell.x, cell.y, cell.z);
        let section_pos = SectionPos::from(at);
        if section.as_ref().is_none_or(|(pos, _)| *pos != section_pos) {
            section = Some((section_pos, world.section(section_pos)));
        }
        let class = match section.as_ref().map(|(_, lookup)| lookup) {
            None | Some(SectionLookup::Outside) => ShapeClass::Empty,
            Some(SectionLookup::Unloaded) => ShapeClass::FullCube,
            Some(SectionLookup::Loaded(blocks)) => {
                table.state(blocks.state(LocalPos::from(at))).class
            }
        };
        let relative = start - cell.as_dvec3();
        if table
            .shape_without_context(class)
            .is_some_and(|shape| shape.any_box(|min, max| segment_hits(relative, delta, min, max)))
        {
            return false;
        }
        if cell == last {
            break;
        }
        if next.x <= next.y && next.x <= next.z {
            cell.x = cell.x.saturating_add(step.x);
            next.x += inverse.x;
        } else if next.y <= next.z {
            cell.y = cell.y.saturating_add(step.y);
            next.y += inverse.y;
        } else {
            cell.z = cell.z.saturating_add(step.z);
            next.z += inverse.z;
        }
    }
    true
}

fn sign(value: f64) -> i32 {
    if value > 0.0 {
        1
    } else if value < 0.0 {
        -1
    } else {
        0
    }
}

/// True when the segment from `start` to `start + delta` meets the box from `min` to `max`, all
/// relative to one block.
fn segment_hits(start: DVec3, delta: DVec3, min: DVec3, max: DVec3) -> bool {
    let mut near = 0.0_f64;
    let mut far = 1.0_f64;
    for ((start, delta), (min, max)) in start
        .to_array()
        .into_iter()
        .zip(delta.to_array())
        .zip(min.to_array().into_iter().zip(max.to_array()))
    {
        if delta == 0.0 {
            if start < min || start > max {
                return false;
            }
            continue;
        }
        let low = (min - start) / delta;
        let high = (max - start) / delta;
        near = near.max(low.min(high));
        far = far.min(low.max(high));
        if near > far {
            return false;
        }
    }
    true
}
