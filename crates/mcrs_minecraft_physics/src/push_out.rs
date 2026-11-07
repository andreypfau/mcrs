use glam::DVec3;
use mcrs_voxel::BlockPos;

use crate::Aabb;
use crate::BlockFlags;
use crate::Collider;
use crate::CollisionTable;
use crate::CollisionWorld;
use crate::PlayerState;
use crate::QueryVariant;
use crate::block_scan::ShapeContext;
use crate::block_scan::entity_shape;
use crate::block_scan::floor_block;
use crate::block_scan::state_at;
use crate::tick_player::dimensions_of;
use crate::tolerance::COLLISION;

/// The speed towards the free face of a blocked cell.
const PUSH_SPEED: f64 = 0.1;

/// The faces of a cell in the order of the test: west, east, north, south.
const FACES: [Face; 4] = [Face::West, Face::East, Face::North, Face::South];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Face {
    West,
    East,
    North,
    South,
}

impl Face {
    fn neighbour(self, cell: BlockPos) -> BlockPos {
        match self {
            Face::West => BlockPos::new(cell.x.saturating_sub(1), cell.y, cell.z),
            Face::East => BlockPos::new(cell.x.saturating_add(1), cell.y, cell.z),
            Face::North => BlockPos::new(cell.x, cell.y, cell.z.saturating_sub(1)),
            Face::South => BlockPos::new(cell.x, cell.y, cell.z.saturating_add(1)),
        }
    }

    fn distance(self, cell: BlockPos, point: DVec3) -> f64 {
        match self {
            Face::West => point.x - f64::from(cell.x),
            Face::East => 1.0 - (point.x - f64::from(cell.x)),
            Face::North => point.z - f64::from(cell.z),
            Face::South => 1.0 - (point.z - f64::from(cell.z)),
        }
    }

    fn push(self, velocity: &mut DVec3) {
        match self {
            Face::West => velocity.x = -PUSH_SPEED,
            Face::East => velocity.x = PUSH_SPEED,
            Face::North => velocity.z = -PUSH_SPEED,
            Face::South => velocity.z = PUSH_SPEED,
        }
    }
}

/// Pushes the player out of the blocks that cause suffocation: for each of four points near the
/// corners of the box, a blocked cell sets the velocity towards its nearest free face.
pub(crate) fn push_out_of_blocks<W: CollisionWorld>(
    world: &W,
    table: &CollisionTable,
    state: &mut PlayerState,
) {
    let dimensions = dimensions_of(state.pose, &state.attributes);
    let bounds = Aabb::of_body(state.position, dimensions.width, dimensions.height);
    let reach = f64::from(dimensions.width) * 0.35;
    let position = state.position;
    let entity = state.query_entity();
    let context = ShapeContext {
        entity: &entity,
        feet: position.y,
        variant: QueryVariant::Collision,
    };
    let column = Column {
        world,
        table,
        context: &context,
        min_y: bounds.min.y,
        max_y: bounds.max.y,
    };
    for (offset_x, offset_z) in [
        (-reach, reach),
        (-reach, -reach),
        (reach, -reach),
        (reach, reach),
    ] {
        let point = DVec3::new(position.x + offset_x, position.y, position.z + offset_z);
        let cell = BlockPos::new(
            floor_block(point.x),
            floor_block(point.y),
            floor_block(point.z),
        );
        if !column.blocked(cell) {
            continue;
        }
        let mut nearest: Option<(Face, f64)> = None;
        for face in FACES {
            if column.blocked(face.neighbour(cell)) {
                continue;
            }
            let distance = face.distance(cell, point);
            if nearest.is_none_or(|(_, best)| distance < best) {
                nearest = Some((face, distance));
            }
        }
        if let Some((face, _)) = nearest {
            face.push(&mut state.velocity);
        }
    }
}

/// The column of a cell between the bottom and the top of the box.
struct Column<'a, W> {
    world: &'a W,
    table: &'a CollisionTable,
    context: &'a ShapeContext<'a>,
    min_y: f64,
    max_y: f64,
}

impl<W: CollisionWorld> Column<'_, W> {
    /// True when a block that causes suffocation meets the column of the cell, shrunk by
    /// `1.0E-7`.
    fn blocked(&self, cell: BlockPos) -> bool {
        let column = Aabb::new(
            DVec3::new(f64::from(cell.x), self.min_y, f64::from(cell.z)),
            DVec3::new(f64::from(cell.x) + 1.0, self.max_y, f64::from(cell.z) + 1.0),
        )
        .deflate(COLLISION);
        (floor_block(column.min.y)..=floor_block(column.max.y)).any(|y| {
            let at = BlockPos::new(cell.x, y, cell.z);
            let data = self.table.state(state_at(self.world, at));
            data.flags.contains(BlockFlags::SUFFOCATES)
                && entity_shape(self.table, data, at, self.context).is_some_and(|shape| {
                    Collider::Block { shape, at }.intersects(self.table, &column)
                })
        })
    }
}
