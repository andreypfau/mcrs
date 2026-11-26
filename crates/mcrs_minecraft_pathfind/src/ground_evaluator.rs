use crate::path_type_table::PathState;
use crate::{
    MobFlags, MobInputs, NodeId, PathQueries, PathTypeTable, SearchRegion, SearchScratch,
    StateFlags,
};
use glam::DVec3;
use mcrs_minecraft_physics::{CollisionWorld, PathType, Tables};
use mcrs_voxel::BlockPos;

pub struct GroundEvaluator<'a, 'w, W: CollisionWorld + 'w> {
    pub(crate) world: &'w W,
    pub(crate) tables: &'a Tables,
    pub(crate) types: &'a PathTypeTable,
    pub(crate) mob: &'a MobInputs,
    pub(crate) region: &'a SearchRegion,
    pub(crate) scratch: &'a mut SearchScratch<W::Section<'w>>,
}
impl<'a, 'w, W: CollisionWorld> GroundEvaluator<'a, 'w, W> {
    pub fn new(
        world: &'w W,
        tables: &'a Tables,
        types: &'a PathTypeTable,
        mob: &'a MobInputs,
        region: &'a SearchRegion,
        scratch: &'a mut SearchScratch<W::Section<'w>>,
    ) -> Option<Self> {
        if !mob.entity.position.is_finite()
            || !mob.bounds.min.is_finite()
            || !mob.bounds.max.is_finite()
            || !mob.bounds.size().is_finite()
            || mob.bounds.size().min_element() <= 0.0
            || !mob.step_height.is_finite()
            || mob.step_height < 0.0
            || mob.fall_limit < 0
            || region.min_y > region.max_y
        {
            return None;
        }
        scratch.clear();
        PathQueries::new(world, types, &mob.path, region, &mut scratch.paths)?;
        Some(Self {
            world,
            tables,
            types,
            mob,
            region,
            scratch,
        })
    }
    pub(crate) fn queries(&mut self) -> PathQueries<'_, 'w, W> {
        PathQueries {
            world: self.world,
            types: self.types,
            mob: &self.mob.path,
            region: self.region,
            scratch: &mut self.scratch.paths,
        }
    }
    pub(crate) fn state(&mut self, at: BlockPos) -> PathState {
        self.queries().state(at)
    }
    pub(crate) fn mob_path_type(&mut self, at: BlockPos) -> PathType {
        self.queries().mob_path_type(at)
    }
    pub fn start(&mut self) -> NodeId {
        let mut at = self.mob.path.block_position;
        if self.mob.path.flags.contains(MobFlags::FLOAT) && self.mob.in_floatable_fluid {
            if self.state(at).flags.contains(StateFlags::FLOATABLE_FLUID) {
                while let Some(y) = at.y.checked_add(1) {
                    let next = BlockPos::new(at.x, y, at.z);
                    if !self.state(next).flags.contains(StateFlags::FLOATABLE_FLUID) {
                        break;
                    }
                    at = next;
                }
            } else {
                at.y = at.y.saturating_sub(1);
            }
        } else if self.mob.on_ground {
            at.y = (self.mob.entity.position.y + 0.5).floor() as i32;
        } else {
            let top = (self.mob.entity.position.y + 1.0).floor() as i32;
            if top > self.region.min_y {
                at.y = top.min(self.region.max_y.saturating_add(1));
                while at.y > self.region.min_y.saturating_add(1) {
                    let below = self.state(at.below()).flags;
                    if !below.contains(StateFlags::AIR)
                        && !below.contains(StateFlags::LAND_PATHFINDABLE)
                    {
                        break;
                    }
                    at = at.below();
                }
            }
        }
        if !self.can_start(at) {
            let bounds = self.mob.bounds;
            for corner in [
                bounds.min,
                DVec3::new(bounds.min.x, 0.0, bounds.max.z),
                DVec3::new(bounds.max.x, 0.0, bounds.min.z),
                bounds.max,
            ] {
                let corner = BlockPos::new(corner.x.floor() as i32, at.y, corner.z.floor() as i32);
                if self.can_start(corner) {
                    at = corner;
                    break;
                }
            }
        }
        let kind = self.mob_path_type(at);
        let id = self.scratch.at(at);
        if let Some(node) = self.scratch.node_mut(id) {
            node.kind = kind;
            node.malus = self.mob.path.malus.get(kind);
        }
        id
    }
    fn can_start(&mut self, at: BlockPos) -> bool {
        let kind = self.mob_path_type(at);
        kind != PathType::Open && self.mob.path.malus.get(kind) >= 0.0
    }
    pub(crate) fn floor(&mut self, at: BlockPos) -> f64 {
        if self.mob.path.flags.contains(MobFlags::FLOAT)
            && self.state(at).flags.contains(StateFlags::FLOATABLE_FLUID)
        {
            f64::from(at.y) + 0.5
        } else {
            f64::from(at.y) - 1.0 + self.state(at.below()).top
        }
    }
    pub fn neighbours(&mut self, id: NodeId, out: &mut [Option<NodeId>; 8]) {
        out.fill(None);
        let Some(node) = self.scratch.node(id) else {
            return;
        };
        let (at, malus) = (node.position, node.malus);
        let kind = self.mob_path_type(at);
        let floor = self.floor(at);
        let above = self.mob_path_type(at.offset(0, 1, 0));
        let jump = if kind != PathType::Sticky && self.mob.path.malus.get(above) >= 0.0 {
            self.mob.step_height.max(1.0).floor() as i32
        } else {
            0
        };
        let sides = [(0, -1), (1, 0), (0, 1), (-1, 0)];
        let candidates =
            sides.map(|side| self.probe(at.offset(side.0, 0, side.1), jump, floor, side, kind));
        let mut accepted = candidates
            .into_iter()
            .filter(|candidate| {
                candidate
                    .and_then(|id| self.scratch.node(id))
                    .is_some_and(|node| !node.closed && (node.malus >= 0.0 || malus < 0.0))
            })
            .flatten();
        for slot in out.iter_mut().take(4) {
            *slot = accepted.next();
        }
        let mut count = out.iter().flatten().count();
        for index in 0..4 {
            let Some((&side, &other)) = sides.get(index).zip(sides.get((index + 1) % 4)) else {
                continue;
            };
            let pair = candidates
                .get(index)
                .copied()
                .flatten()
                .zip(candidates.get((index + 1) % 4).copied().flatten());
            if !self.diagonal(at, pair) {
                continue;
            }
            let candidate = self.probe(
                at.offset(side.0 + other.0, 0, side.1 + other.1),
                jump,
                floor,
                side,
                kind,
            );
            if candidate
                .and_then(|id| self.scratch.node(id))
                .is_some_and(|n| !n.closed && n.kind != PathType::WalkableDoor && n.malus >= 0.0)
                && let Some(slot) = out.get_mut(count)
            {
                *slot = candidate;
                count = count.saturating_add(1);
            }
        }
    }
    fn diagonal(&self, at: BlockPos, pair: Option<(NodeId, NodeId)>) -> bool {
        let Some((a, b)) = pair.and_then(|(a, b)| self.scratch.node(a).zip(self.scratch.node(b)))
        else {
            return false;
        };
        let narrow_fences =
            self.mob.path.width < 0.5 && a.kind == PathType::Fence && b.kind == PathType::Fence;
        [a, b].into_iter().all(|n| {
            n.position.y <= at.y
                && n.kind != PathType::WalkableDoor
                && (self.mob.path.width <= 1.0 || n.malus <= 0.0)
                && (n.position.y < at.y || n.malus >= 0.0 || narrow_fences)
        })
    }
}
