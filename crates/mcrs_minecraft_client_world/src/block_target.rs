use bevy::ecs::entity::Entity;
use bevy::math::{DVec3, IVec3};
use mcrs_minecraft_core::Direction;
use mcrs_minecraft_core::voxel_shape::Aabb;
use mcrs_minecraft_entity_attribute::AttributeInstances;
use mcrs_minecraft_entity_attribute::attribute_keys::Attribute;
use mcrs_voxel::BlockPos;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Reach {
    pub block: f64,
    pub entity: f64,
}

impl Reach {
    #[must_use]
    pub fn from_instances(instances: &AttributeInstances) -> Option<Reach> {
        Some(Reach {
            block: instances.value(Attribute::BlockInteractionRange)?,
            entity: instances.value(Attribute::EntityInteractionRange)?,
        })
    }

    fn ray_length(self) -> f64 {
        self.block.max(self.entity)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BlockHit {
    pub pos: BlockPos,
    pub face: Direction,
    pub point: DVec3,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EntityBox {
    pub entity: Entity,
    pub min: DVec3,
    pub max: DVec3,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EntityHit {
    pub entity: Entity,
    pub point: DVec3,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Target {
    Block(BlockHit),
    Entity(EntityHit),
}

#[must_use]
pub fn target<'s>(
    eye: DVec3,
    direction: DVec3,
    reach: Reach,
    outline_at: impl Fn(BlockPos) -> &'s [Aabb],
    entities: impl IntoIterator<Item = EntityBox>,
) -> Option<Target> {
    let block = cast(eye, direction, reach.ray_length(), outline_at);
    let limit = block.map_or(reach.ray_length(), |hit| hit.point.distance(eye));
    let mut nearest: Option<(f64, Entity)> = None;
    for candidate in entities {
        let Some((t, _)) = enter(eye, direction, candidate.min, candidate.max) else {
            continue;
        };
        if t < limit && nearest.is_none_or(|(best, _)| t < best) {
            nearest = Some((t, candidate.entity));
        }
    }
    match (nearest, block) {
        (Some((t, entity)), _) => (t < reach.entity).then(|| {
            Target::Entity(EntityHit {
                entity,
                point: eye + direction * t,
            })
        }),
        (None, Some(hit)) => (hit.point.distance(eye) < reach.block).then_some(Target::Block(hit)),
        (None, None) => None,
    }
}

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
    use mcrs_minecraft_entity::entity_type_keys::EntityType;
    use mcrs_minecraft_entity_attribute::{AttributeModifier, ModifierOperation};
    use mcrs_minecraft_protocol_game::clientbound::AttributeSnapshot;
    use mcrs_minecraft_resource::rl;

    const FULL: &[Aabb] = &[Aabb {
        min: Vec3::ZERO,
        max: Vec3::ONE,
    }];
    const BOTTOM_SLAB: &[Aabb] = &[Aabb {
        min: Vec3::ZERO,
        max: Vec3::new(1.0, 0.5, 1.0),
    }];

    fn player() -> AttributeInstances {
        AttributeInstances::new(
            mcrs_minecraft_entity::attribute_bases(EntityType::Player)
                .expect("a player has attributes"),
        )
    }

    fn reach(instances: &AttributeInstances) -> Reach {
        Reach::from_instances(instances).expect("a player has both ranges")
    }

    fn creative() -> AttributeInstances {
        let mut instances = player();
        for entry in [
            AttributeSnapshot {
                attribute: Attribute::BlockInteractionRange.into(),
                base: 4.5,
                modifiers: vec![AttributeModifier {
                    id: rl!("minecraft:creative_mode_block_range"),
                    amount: 0.5,
                    operation: ModifierOperation::AddValue,
                }],
            },
            AttributeSnapshot {
                attribute: Attribute::EntityInteractionRange.into(),
                base: 3.0,
                modifiers: vec![AttributeModifier {
                    id: rl!("minecraft:creative_mode_entity_range"),
                    amount: 2.0,
                    operation: ModifierOperation::AddValue,
                }],
            },
        ] {
            let _change = instances
                .receive(&entry)
                .expect("each range is in the registry");
        }
        instances
    }

    fn block_target<'s>(
        eye: DVec3,
        direction: DVec3,
        reach: Reach,
        outline_at: impl Fn(BlockPos) -> &'s [Aabb],
    ) -> Option<BlockHit> {
        match target(eye, direction, reach, outline_at, []) {
            Some(Target::Block(hit)) => Some(hit),
            Some(Target::Entity(_)) | None => None,
        }
    }

    fn one_block(at: BlockPos, shape: &'static [Aabb]) -> impl Fn(BlockPos) -> &'static [Aabb] {
        move |pos| if pos == at { shape } else { &[] }
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum Picked {
        Entity,
        Block,
        Miss,
    }

    struct PickRow {
        name: &'static str,
        entity_front: f64,
        block_z: i32,
        look: DVec3,
        picked: Picked,
    }

    const PICK_ROWS: &[PickRow] = &[
        PickRow {
            name: "an entity nearer than the block",
            entity_front: 2.0,
            block_z: 3,
            look: DVec3::Z,
            picked: Picked::Entity,
        },
        PickRow {
            name: "a block nearer than the entity",
            entity_front: 2.5,
            block_z: 2,
            look: DVec3::Z,
            picked: Picked::Block,
        },
        PickRow {
            name: "an entity just inside the entity range",
            entity_front: 2.999,
            block_z: 10,
            look: DVec3::Z,
            picked: Picked::Entity,
        },
        PickRow {
            name: "an entity at the entity range",
            entity_front: 3.0,
            block_z: 10,
            look: DVec3::Z,
            picked: Picked::Miss,
        },
        PickRow {
            name: "an entity out of range before a block in range",
            entity_front: 3.2,
            block_z: 4,
            look: DVec3::Z,
            picked: Picked::Miss,
        },
        PickRow {
            name: "a look that misses both",
            entity_front: 2.0,
            block_z: 3,
            look: DVec3::Y,
            picked: Picked::Miss,
        },
    ];

    #[test]
    fn cbk_tgt_009_the_nearer_hit_is_the_target_within_its_range() {
        let eye = DVec3::new(0.5, 0.5, 0.0);
        for row in PICK_ROWS {
            let pig = EntityBox {
                entity: Entity::PLACEHOLDER,
                min: DVec3::new(0.05, 0.0, row.entity_front),
                max: DVec3::new(0.95, 0.9, row.entity_front + 0.9),
            };
            let picked = match target(
                eye,
                row.look,
                reach(&player()),
                one_block(BlockPos::new(0, 0, row.block_z), FULL),
                [pig],
            ) {
                Some(Target::Entity(hit)) => {
                    assert!(
                        (hit.point.z - row.entity_front).abs() < 1e-9,
                        "{}: the hit point is on the front of the box",
                        row.name
                    );
                    Picked::Entity
                }
                Some(Target::Block(_)) => Picked::Block,
                None => Picked::Miss,
            };
            assert_eq!(picked, row.picked, "{}", row.name);
        }
    }

    #[test]
    fn cbk_tgt_007_the_face_is_the_side_the_ray_goes_in_through() {
        let block = BlockPos::new(3, 64, -2);
        let eye = DVec3::new(0.5, 66.62, -1.5);
        let direction = (DVec3::new(3.5, 65.0, -1.5) - eye).normalize();
        let hit = block_target(eye, direction, reach(&creative()), one_block(block, FULL))
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
        let reach = reach(&creative());
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
        let survival = reach(&player());
        let creative = reach(&creative());

        assert!(block_target(eye(0.01), north, survival, one_block(block, FULL)).is_none());
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
            reach(&creative()),
            one_block(block, FULL),
        )
        .expect("the eye is inside the block");
        assert_eq!((hit.face, hit.point), (Direction::Up, eye));
    }
}
