use crate::{GroundEvaluator, MobFlags, NodeId};
use glam::DVec3;
use mcrs_minecraft_physics::{Aabb, CollisionWorld, PathType};
use mcrs_voxel::BlockPos;

impl<W: CollisionWorld> GroundEvaluator<'_, '_, W> {
    pub(crate) fn probe(
        &mut self,
        mut at: BlockPos,
        mut jump: i32,
        floor: f64,
        side: (i32, i32),
        from: PathType,
    ) -> Option<NodeId> {
        let start_y = at.y;
        let id = loop {
            if self.floor(at) - floor > 1.125_f64.max(f64::from(self.mob.step_height)) {
                return None;
            }
            let kind = self.mob_path_type(at);
            let mut candidate =
                (self.mob.path.malus.get(kind) >= 0.0).then(|| self.accept(at, kind));
            if partial(from) && candidate.is_some() && self.sweep(at) {
                candidate = None;
            }
            if kind == PathType::Walkable {
                break candidate?;
            }
            if candidate.is_none()
                && jump > 0
                && (kind != PathType::Fence
                    || self.mob.path.flags.contains(MobFlags::WALK_OVER_FENCES))
                && !matches!(
                    kind,
                    PathType::UnpassableRail | PathType::DropDown | PathType::PowderSnow
                )
            {
                let next = at.offset(0, 1, 0);
                if next.y <= at.y {
                    return None;
                }
                at = next;
                jump = jump.saturating_sub(1);
                continue;
            }
            if kind == PathType::Water && !self.mob.path.flags.contains(MobFlags::FLOAT) {
                let mut below = at.below();
                while below.y > self.region.min_y && self.mob_path_type(below) == PathType::Water {
                    candidate = (self.mob.path.malus.get(PathType::Water) >= 0.0)
                        .then(|| self.accept(below, PathType::Water));
                    below = below.below();
                }
            } else if kind == PathType::Open {
                break self.fall(at);
            } else if partial(kind) && candidate.is_none() {
                let id = self.scratch.at(at);
                if let Some(node) = self.scratch.node_mut(id) {
                    node.kind = kind;
                    node.malus = kind.default_malus();
                    node.closed = true;
                }
                candidate = Some(id);
            }
            break candidate?;
        };
        while at.y > start_y {
            at = at.below();
            self.step_up(at, id, side)?;
        }
        Some(id)
    }
    fn accept(&mut self, at: BlockPos, kind: PathType) -> NodeId {
        let id = self.scratch.at(at);
        if let Some(node) = self.scratch.node_mut(id) {
            node.kind = kind;
            node.malus = node.malus.max(self.mob.path.malus.get(kind));
        }
        id
    }
    fn step_up(&mut self, at: BlockPos, id: NodeId, side: (i32, i32)) -> Option<NodeId> {
        let node = self.scratch.node(id)?;
        if self.mob.path.width >= 1.0 || !matches!(node.kind, PathType::Open | PathType::Walkable) {
            return Some(id);
        }
        let result = node.position;
        let cx = f64::from(at.x) - f64::from(side.0) + 0.5;
        let cz = f64::from(at.z) - f64::from(side.1) + 0.5;
        let half = f64::from(self.mob.path.width) / 2.0;
        let f1 = self.floor(BlockPos::from(DVec3::new(cx, f64::from(at.y) + 1.0, cz)));
        let f2 = self.floor(result);
        let bounds = Aabb::new(
            DVec3::new(cx - half, f1 + 0.001, cz - half),
            DVec3::new(
                cx + half,
                f64::from(self.mob.path.height) + f2 - 0.002,
                cz + half,
            ),
        );
        (!self.collides(bounds)).then_some(id)
    }
    fn fall(&mut self, at: BlockPos) -> NodeId {
        let mut below = at;
        while below.y > self.region.min_y {
            below = below.below();
            if i64::from(at.y) - i64::from(below.y) > i64::from(self.mob.fall_limit) {
                return self.blocked(below);
            }
            let kind = self.mob_path_type(below);
            if kind == PathType::Open {
                continue;
            }
            return if self.mob.path.malus.get(kind) >= 0.0 {
                self.accept(below, kind)
            } else {
                self.blocked(below)
            };
        }
        self.blocked(at)
    }
    fn blocked(&mut self, at: BlockPos) -> NodeId {
        let id = self.scratch.at(at);
        if let Some(node) = self.scratch.node_mut(id) {
            node.kind = PathType::Blocked;
            node.malus = -1.0;
        }
        id
    }
    fn sweep(&mut self, at: BlockPos) -> bool {
        let size = self.mob.bounds.size();
        let position = self.mob.entity.position;
        let mut delta = DVec3::new(
            f64::from(at.x) - position.x + size.x / 2.0,
            f64::from(at.y) - position.y + size.y / 2.0,
            f64::from(at.z) - position.z + size.z / 2.0,
        );
        let distance = (delta.x * delta.x + delta.y * delta.y + delta.z * delta.z).sqrt();
        let steps = (distance / ((size.x + size.y + size.z) / 3.0)).ceil() as i32;
        if steps == 0 {
            return false;
        }
        delta *= f64::from(1.0_f32 / steps as f32);
        let mut bounds = self.mob.bounds;
        (0..steps).any(|_| {
            bounds = bounds.translate(delta);
            self.collides(bounds)
        })
    }
}
#[must_use]
fn partial(kind: PathType) -> bool {
    matches!(
        kind,
        PathType::Fence | PathType::DoorWoodClosed | PathType::DoorIronClosed
    )
}
