use bevy::math::{DVec3, IVec3};
use mcrs_minecraft_core::Direction;
use mcrs_minecraft_core::voxel_shape::Aabb;
use mcrs_minecraft_game_mode::GameMode;
use mcrs_voxel::BlockPos;

/// The ranges of the player, as the attributes `minecraft:block_interaction_range` and
/// `minecraft:entity_interaction_range` give them.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Reach {
    pub block: f64,
    pub entity: f64,
}

impl Reach {
    const BASE: Reach = Reach {
        block: 4.5,
        entity: 3.0,
    };
    const CREATIVE: Reach = Reach {
        block: 5.0,
        entity: 5.0,
    };

    /// The client does not read `update_attributes` yet, so it takes the values that a vanilla
    /// server gives each game mode: the base values, and in creative mode the creative range
    /// modifiers added to them.
    #[must_use]
    pub fn of(mode: GameMode) -> Reach {
        match mode {
            GameMode::Creative => Reach::CREATIVE,
            _ => Reach::BASE,
        }
    }

    fn ray_length(self) -> f64 {
        self.block.max(self.entity)
    }
}

/// A block that the ray of the player hits.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BlockHit {
    pub pos: BlockPos,
    pub face: Direction,
    pub point: DVec3,
}

/// The block target of a ray from `eye` along the unit vector `direction`, or `None` for a miss.
/// `outline_at` gives the outline shape of the block at a position, in the coordinates of that
/// position. The client tests no entities, so no entity target hides a block.
#[must_use]
pub fn block_target<'s>(
    eye: DVec3,
    direction: DVec3,
    reach: Reach,
    outline_at: impl Fn(BlockPos) -> &'s [Aabb],
) -> Option<BlockHit> {
    let hit = cast(eye, direction, reach.ray_length(), outline_at)?;
    (hit.point.distance(eye) < reach.block).then_some(hit)
}

/// Visits the positions that the ray crosses, from the one that holds `eye`, and answers the
/// first hit on an outline shape.
fn cast<'s>(
    eye: DVec3,
    direction: DVec3,
    length: f64,
    outline_at: impl Fn(BlockPos) -> &'s [Aabb],
) -> Option<BlockHit> {
    let mut cell = eye.floor().as_ivec3();
    let step = IVec3::new(
        axis_step(direction.x),
        axis_step(direction.y),
        axis_step(direction.z),
    );
    let next_boundary = |axis: usize, cell: IVec3| {
        let d = direction[axis];
        if d == 0.0 {
            return f64::INFINITY;
        }
        let boundary = f64::from(cell[axis] + i32::from(d > 0.0));
        (boundary - eye[axis]) / d
    };
    let mut t_max = DVec3::new(
        next_boundary(0, cell),
        next_boundary(1, cell),
        next_boundary(2, cell),
    );
    let t_delta = direction.abs().recip();
    loop {
        let pos = BlockPos::from(cell);
        if let Some(hit) = clip(eye, direction, length, pos, outline_at(pos)) {
            return Some(hit);
        }
        let axis = t_max.min_position();
        if t_max[axis] > length {
            return None;
        }
        cell[axis] += step[axis];
        t_max[axis] += t_delta[axis];
    }
}

fn axis_step(d: f64) -> i32 {
    if d > 0.0 {
        1
    } else if d < 0.0 {
        -1
    } else {
        0
    }
}

/// The nearest hit of the ray on the boxes of one position, within `length`.
fn clip(
    eye: DVec3,
    direction: DVec3,
    length: f64,
    pos: BlockPos,
    boxes: &[Aabb],
) -> Option<BlockHit> {
    let origin = pos.as_ivec3().as_dvec3();
    let mut nearest: Option<(f64, Direction)> = None;
    for outline in boxes {
        let min = origin + outline.min.as_dvec3();
        let max = origin + outline.max.as_dvec3();
        let Some((t, face)) = enter(eye, direction, min, max) else {
            continue;
        };
        if t <= length && nearest.is_none_or(|(best, _)| t < best) {
            nearest = Some((t, face));
        }
    }
    nearest.map(|(t, face)| BlockHit {
        pos,
        face,
        point: eye + direction * t,
    })
}

/// Where the ray goes into the box, and through which side. A ray that starts inside the box
/// enters at the eye, through the side that points against the main axis of the ray.
fn enter(eye: DVec3, direction: DVec3, min: DVec3, max: DVec3) -> Option<(f64, Direction)> {
    if eye.cmpge(min).all() && eye.cmple(max).all() {
        return Some((
            0.0,
            facing_against(direction, direction.abs().max_position()),
        ));
    }
    let mut t_enter = f64::NEG_INFINITY;
    let mut t_exit = f64::INFINITY;
    let mut enter_axis = 0;
    for axis in 0..3 {
        let d = direction[axis];
        if d == 0.0 {
            if eye[axis] < min[axis] || eye[axis] > max[axis] {
                return None;
            }
            continue;
        }
        let (near, far) = if d > 0.0 {
            (min[axis], max[axis])
        } else {
            (max[axis], min[axis])
        };
        let t_near = (near - eye[axis]) / d;
        let t_far = (far - eye[axis]) / d;
        if t_near > t_enter {
            t_enter = t_near;
            enter_axis = axis;
        }
        t_exit = t_exit.min(t_far);
    }
    (t_enter >= 0.0 && t_enter <= t_exit).then(|| (t_enter, facing_against(direction, enter_axis)))
}

fn facing_against(direction: DVec3, axis: usize) -> Direction {
    let positive = direction[axis] > 0.0;
    match (axis, positive) {
        (0, true) => Direction::West,
        (0, false) => Direction::East,
        (1, true) => Direction::Down,
        (1, false) => Direction::Up,
        (_, true) => Direction::North,
        (_, false) => Direction::South,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::math::Vec3;

    const FULL: &[Aabb] = &[Aabb {
        min: Vec3::ZERO,
        max: Vec3::ONE,
    }];
    const BOTTOM_SLAB: &[Aabb] = &[Aabb {
        min: Vec3::ZERO,
        max: Vec3::new(1.0, 0.5, 1.0),
    }];

    fn one_block(at: BlockPos, shape: &'static [Aabb]) -> impl Fn(BlockPos) -> &'static [Aabb] {
        move |pos| if pos == at { shape } else { &[] }
    }

    #[test]
    fn cbk_tgt_007_the_face_is_the_side_the_ray_goes_in_through() {
        let block = BlockPos::new(3, 64, -2);
        let eye = DVec3::new(0.5, 66.62, -1.5);
        let direction = (DVec3::new(3.5, 65.0, -1.5) - eye).normalize();
        let hit = block_target(
            eye,
            direction,
            Reach::of(GameMode::Creative),
            one_block(block, FULL),
        )
        .expect("the ray reaches the top of the block");
        assert_eq!((hit.pos, hit.face), (block, Direction::Up));
        assert!((hit.point.y - 65.0).abs() < 1e-9, "{:?}", hit.point);
    }

    #[test]
    fn cbk_tgt_005_the_ray_goes_through_the_empty_part_of_a_cell() {
        let slab = BlockPos::new(1, 0, 0);
        let behind = BlockPos::new(2, 0, 0);
        let outline_at = |pos: BlockPos| match pos {
            pos if pos == slab => BOTTOM_SLAB,
            pos if pos == behind => FULL,
            _ => &[],
        };
        let reach = Reach::of(GameMode::Creative);
        let east = DVec3::X;

        let above_the_slab = block_target(DVec3::new(0.5, 0.75, 0.5), east, reach, outline_at);
        assert_eq!(
            above_the_slab.map(|hit| (hit.pos, hit.face)),
            Some((behind, Direction::West))
        );
        let into_the_slab = block_target(DVec3::new(0.5, 0.25, 0.5), east, reach, outline_at);
        assert_eq!(into_the_slab.map(|hit| hit.pos), Some(slab));
    }

    #[test]
    fn cbk_tgt_010_the_range_limits_the_hit_point_and_not_the_block_cube() {
        let block = BlockPos::new(0, 0, 5);
        let north = DVec3::Z;
        let eye = |z: f64| DVec3::new(0.5, 0.25, z);
        let creative = Reach::of(GameMode::Creative);
        let survival = Reach::of(GameMode::Survival);

        assert!(block_target(eye(0.01), north, creative, one_block(block, FULL)).is_some());
        assert!(block_target(eye(0.0), north, creative, one_block(block, FULL)).is_none());
        assert!(block_target(eye(0.0), north, survival, one_block(block, FULL)).is_none());
        assert!(block_target(eye(0.51), north, survival, one_block(block, FULL)).is_some());
    }

    #[test]
    fn cbk_tgt_007_an_eye_inside_the_outline_hits_against_the_main_axis() {
        let block = BlockPos::new(0, 0, 0);
        let eye = DVec3::new(0.5, 0.5, 0.5);
        let down_and_east = DVec3::new(0.3, -0.9, 0.1).normalize();
        let hit = block_target(
            eye,
            down_and_east,
            Reach::of(GameMode::Creative),
            one_block(block, FULL),
        )
        .expect("the eye is inside the block");
        assert_eq!((hit.face, hit.point), (Direction::Up, eye));
    }
}
