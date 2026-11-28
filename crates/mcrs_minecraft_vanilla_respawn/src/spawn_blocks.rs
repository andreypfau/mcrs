use alloc::vec::Vec;
use glam::DVec3;
use mcrs_minecraft_collision_table::BlockWorld;
use mcrs_minecraft_entity_key::EntityUuid;
use mcrs_minecraft_physics::Aabb;
use mcrs_minecraft_physics::Axis;
use mcrs_minecraft_physics::BlockFlags;
use mcrs_minecraft_physics::CollisionTable;
use mcrs_minecraft_physics::NoBodies;
use mcrs_minecraft_physics::QueryEntity;
use mcrs_minecraft_physics::Shape;
use mcrs_minecraft_physics::ShapeClass;
use mcrs_minecraft_physics::StateCollision;
use mcrs_minecraft_physics::Tables;
use mcrs_minecraft_physics::View;
use mcrs_minecraft_physics::intersects;
use mcrs_minecraft_pose::Pose;
use mcrs_minecraft_volume::BlockVolume;
use mcrs_voxel::BlockPos;

#[derive(Clone, Copy, Debug)]
pub(crate) struct SpawnColumnHeights {
    pub(crate) motion: i32,
    pub(crate) surface: i32,
    pub(crate) ocean_floor: i32,
}

pub(crate) struct SpawnBlocks<'a, V> {
    pub(crate) world: BlockWorld<'a, V>,
    pub(crate) tables: &'a Tables,
    pub(crate) min_y: i32,
    pub(crate) max_y: i32,
}

impl<V: BlockVolume> SpawnBlocks<'_, V> {
    #[must_use]
    pub(crate) fn height_fix(&self, spawn: BlockPos) -> DVec3 {
        let mut y = spawn.y;
        while y < self.max_y && !self.is_clear(at(spawn.x, y, spawn.z)) {
            y += 1;
        }
        y = y.saturating_sub(1);
        while y > self.min_y && self.is_clear(at(spawn.x, y, spawn.z)) {
            y -= 1;
        }
        at(spawn.x, y.saturating_add(1), spawn.z)
    }

    #[must_use]
    pub(crate) fn column(&self, x: i32, z: i32, heights: SpawnColumnHeights) -> Option<DVec3> {
        let top = heights.motion;
        if top < self.min_y {
            return None;
        }
        if heights.surface <= top && heights.surface > heights.ocean_floor {
            return None;
        }
        let start = top.saturating_add(1).min(self.max_y);
        for y in (self.min_y..=start).rev() {
            let state = self.state(BlockPos::new(x, y, z))?;
            if state.flags.contains(BlockFlags::FLUID) {
                return None;
            }
            if has_full_top(self.world.table, state) {
                let feet = at(x, y.saturating_add(1), z);
                return self.is_clear(feet).then_some(feet);
            }
        }
        None
    }

    fn state(&self, pos: BlockPos) -> Option<StateCollision> {
        let state = self.world.blocks.block(pos).ok()?;
        Some(self.world.table.state(state))
    }

    fn is_clear(&self, feet: DVec3) -> bool {
        !self.collides(feet) && !self.has_liquid(feet)
    }

    fn collides(&self, feet: DVec3) -> bool {
        let entity = QueryEntity {
            key: EntityUuid::default(),
            position: feet,
            fall_distance: 0.0,
            shift: false,
            powder_snow_solid: false,
            walks_on_powder_snow: false,
        };
        let view = View::new(&self.world, &NoBodies, self.tables);
        intersects(view, &entity, player_box(feet), &mut Vec::new())
    }

    fn has_liquid(&self, feet: DVec3) -> bool {
        let bounds = player_box(feet);
        let low = floor_pos(bounds.min);
        let high = ceil_pos(bounds.max);
        (low.y..high.y).any(|y| {
            (low.x..high.x).any(|x| {
                (low.z..high.z).any(|z| {
                    self.state(BlockPos::new(x, y, z))
                        .is_some_and(|state| state.flags.contains(BlockFlags::LIQUID_BLOCK))
                })
            })
        })
    }
}

#[must_use]
pub(crate) fn lower_face_centre(pos: BlockPos) -> DVec3 {
    at(pos.x, pos.y, pos.z)
}

fn at(x: i32, y: i32, z: i32) -> DVec3 {
    DVec3::new(f64::from(x) + 0.5, f64::from(y), f64::from(z) + 0.5)
}

fn player_box(feet: DVec3) -> Aabb {
    let dimensions = Pose::Standing.player_dimensions();
    Aabb::of_body(feet, dimensions.width, dimensions.height)
}

fn floor_pos(value: DVec3) -> BlockPos {
    BlockPos::new(
        libm::floor(value.x) as i32,
        libm::floor(value.y) as i32,
        libm::floor(value.z) as i32,
    )
}

fn ceil_pos(value: DVec3) -> BlockPos {
    BlockPos::new(
        libm::ceil(value.x) as i32,
        libm::ceil(value.y) as i32,
        libm::ceil(value.z) as i32,
    )
}

fn has_full_top(table: &CollisionTable, state: StateCollision) -> bool {
    match state.class {
        ShapeClass::FullCube => true,
        ShapeClass::OneBox(id) | ShapeClass::Compound(id) => table.shape(id).is_some_and(full_top),
        ShapeClass::Empty | ShapeClass::Scaffolding { .. } | ShapeClass::PowderSnow => false,
    }
}

fn full_top(shape: &Shape) -> bool {
    let xs = shape.lines(Axis::X);
    let ys = shape.lines(Axis::Y);
    let zs = shape.lines(Axis::Z);
    let spans = |lines: &[f64]| lines.first() == Some(&0.0) && lines.last() == Some(&1.0);
    if !spans(xs) || !spans(zs) || ys.last() != Some(&1.0) {
        return false;
    }
    let Some(top) = ys.len().checked_sub(2) else {
        return false;
    };
    let cells_x = xs.len().saturating_sub(1);
    let cells_z = zs.len().saturating_sub(1);
    (0..cells_x).all(|x| (0..cells_z).all(|z| shape.is_solid(x, top, z)))
}
