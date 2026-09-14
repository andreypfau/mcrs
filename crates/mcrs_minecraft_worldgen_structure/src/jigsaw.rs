use std::collections::{BTreeMap, VecDeque};

use crate::{JigsawConfig, TerrainAdaptation};
use bevy_math::IVec3;
use mcrs_minecraft_core::ResourceLocation;
use mcrs_minecraft_core::Rotation;
use mcrs_minecraft_core::{BlockPos, BoundingBox};
use mcrs_minecraft_random::legacy::LegacyRandom;
use mcrs_minecraft_random::{shuffle, shuffled};
use mcrs_minecraft_worldgen_feature::placement::HeightmapName;
use mcrs_minecraft_worldgen_feature::template::Joint;
use mcrs_minecraft_worldgen_feature::template::Projection;

use super::frozen::{ElementId, FrozenStructures, PoolId, StructureId, StructureKind};
use super::site::{PlacedJigsaw, Site, SiteWorld, element_bounds, shuffled_jigsaws};

pub const TERRAIN_MARGIN: i32 = 12;
const EMPTY_POOL: &str = "minecraft:empty";
const EXPANSION_HACK_MAX_HEIGHT: i32 = 16;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Junction {
    pub source_x: i32,
    pub source_ground_y: i32,
    pub source_z: i32,
    pub delta_y: i32,
    pub dest_projection: Projection,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JigsawPiece {
    pub element: ElementId,
    pub position: IVec3,
    pub rotation: Rotation,
    pub bounds: BoundingBox,
    pub projection: Projection,
    pub ground_level_delta: i32,
    pub junctions: Vec<Junction>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Piece {
    Jigsaw(JigsawPiece),
}

impl Piece {
    pub fn bounds(&self) -> BoundingBox {
        match self {
            Piece::Jigsaw(piece) => piece.bounds,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Start {
    pub structure: StructureId,
    pub pieces: Vec<Piece>,
    pub bounds: BoundingBox,
}

impl Start {
    pub fn new(frozen: &FrozenStructures, structure: StructureId, pieces: Vec<Piece>) -> Self {
        let union = pieces
            .iter()
            .map(Piece::bounds)
            .reduce(BoundingBox::union)
            .expect("a start has at least one piece");
        let bounds =
            if frozen.structures[structure.0 as usize].adaptation == TerrainAdaptation::None {
                union
            } else {
                union.inflated(TERRAIN_MARGIN)
            };
        Start {
            structure,
            pieces,
            bounds,
        }
    }
}

/// Inclusive integer arithmetic is exact for the reference's quarter-deflated
/// voxel test.
#[derive(Debug, Clone, PartialEq, Eq)]
struct FreeSpace {
    bounds: BoundingBox,
    holes: Vec<BoundingBox>,
}

impl FreeSpace {
    fn fits(&self, candidate: &BoundingBox) -> bool {
        self.bounds.contains(*candidate)
            && !self.holes.iter().any(|hole| hole.intersects(*candidate))
    }
}

struct Pending {
    piece: usize,
    space: usize,
    depth: i32,
}

#[derive(Default)]
struct PriorityQueue(BTreeMap<i32, VecDeque<Pending>>);

impl PriorityQueue {
    fn push(&mut self, priority: i32, pending: Pending) {
        self.0.entry(priority).or_default().push_back(pending);
    }

    fn pop(&mut self) -> Option<Pending> {
        let mut highest = self.0.last_entry()?;
        let next = highest.get_mut().pop_front();
        if highest.get().is_empty() {
            highest.remove();
        }
        next
    }
}

struct Assembly<'a> {
    frozen: &'a FrozenStructures,
    config: &'a JigsawConfig,
    aliases: &'a BTreeMap<ResourceLocation, PoolId>,
    world: &'a mut dyn SiteWorld,
    rng: LegacyRandom,
    pieces: Vec<JigsawPiece>,
    spaces: Vec<FreeSpace>,
    queue: PriorityQueue,
}

pub fn layout(
    frozen: &FrozenStructures,
    structure: StructureId,
    site: Site,
    accessor_min_y: i32,
    accessor_height: i32,
    world: &mut dyn SiteWorld,
) -> Vec<JigsawPiece> {
    let StructureKind::Jigsaw { config, .. } = &frozen.structures[structure.0 as usize].kind else {
        return Vec::new();
    };
    let centre = site.centre;
    let centre_piece = JigsawPiece {
        element: centre.element,
        position: centre.position,
        rotation: centre.rotation,
        bounds: centre.bounds,
        projection: frozen.elements[centre.element.0 as usize]
            .projection()
            .expect("the site rejects an empty centre"),
        ground_level_delta: centre.ground_level_delta,
        junctions: Vec::new(),
    };

    let reach = &config.max_distance_from_center;
    let padding = &config.dimension_padding;
    let c = site.position;
    let bounds = BoundingBox {
        min: BlockPos::new(
            c.x - reach.horizontal(),
            (c.y - reach.vertical()).max(accessor_min_y + padding.bottom()),
            c.z - reach.horizontal(),
        ),
        max: BlockPos::new(
            c.x + reach.horizontal(),
            (c.y + reach.vertical()).min(accessor_min_y + accessor_height - 1 - padding.top()),
            c.z + reach.horizontal(),
        ),
    };
    let mut assembly = Assembly {
        frozen,
        config,
        aliases: &site.aliases,
        world,
        rng: site.rng,
        pieces: vec![centre_piece],
        spaces: vec![FreeSpace {
            bounds,
            holes: vec![centre.bounds],
        }],
        queue: PriorityQueue::default(),
    };
    if config.size.0 > 0 {
        assembly.attach_children(0, 0, 0);
        while let Some(pending) = assembly.queue.pop() {
            assembly.attach_children(pending.piece, pending.space, pending.depth);
        }
    }
    assembly.pieces
}

fn can_attach(source: &PlacedJigsaw<'_>, target: &PlacedJigsaw<'_>) -> bool {
    source.front == target.front.opposite()
        && (source.block.joint == Joint::Rollable || source.top == target.top)
        && target
            .name()
            .is_none_or(|name| source.block.target == *name)
}

impl Assembly<'_> {
    fn pool(&self, name: &ResourceLocation) -> Option<PoolId> {
        self.aliases
            .get(name)
            .or_else(|| self.frozen.pool_ids.get(name))
            .copied()
    }

    fn pool_is_usable(&self, pool: PoolId) -> bool {
        let pool = &self.frozen.pools[pool.0 as usize];
        !pool.expanded.is_empty() || pool.id.as_str() == EMPTY_POOL
    }

    fn expansion_of(
        &self,
        candidate: ElementId,
        rotation: Rotation,
        jigsaws: &[PlacedJigsaw<'_>],
    ) -> i32 {
        if !self.config.use_expansion_hack {
            return 0;
        }
        let hack_box = element_bounds(self.frozen, candidate, IVec3::ZERO, rotation)
            .expect("an empty element never reaches the candidate loop");
        if hack_box.y_span() > EXPANSION_HACK_MAX_HEIGHT {
            return 0;
        }
        jigsaws
            .iter()
            .map(|jigsaw| {
                if !hack_box.is_inside((jigsaw.pos + jigsaw.front.normal()).into()) {
                    return 0;
                }
                let Some(pool) = self.pool(&jigsaw.block.pool) else {
                    return 0;
                };
                let pool = &self.frozen.pools[pool.0 as usize];
                pool.max_size
                    .max(self.frozen.pools[pool.fallback.0 as usize].max_size)
            })
            .max()
            .unwrap_or(0)
    }

    fn attach_children(&mut self, source: usize, space: usize, depth: i32) {
        let frozen = self.frozen;
        let size = self.config.size.0;
        let source_piece = &self.pieces[source];
        let (source_element, source_position, source_rotation) = (
            source_piece.element,
            source_piece.position,
            source_piece.rotation,
        );
        let source_projection = source_piece.projection;
        let source_rigid = source_projection == Projection::Rigid;
        let source_bounds = source_piece.bounds;
        let source_box_y = source_bounds.min.y;
        let source_ground_level_delta = source_piece.ground_level_delta;
        let mut interior: Option<usize> = None;

        let source_jigsaws = shuffled_jigsaws(
            frozen,
            source_element,
            source_position,
            source_rotation,
            &mut self.rng,
        );
        for source_jigsaw in &source_jigsaws {
            let front = source_jigsaw.front;
            let source_jigsaw_pos = source_jigsaw.pos;
            let target_jigsaw_pos = source_jigsaw_pos + front.normal();
            let source_jigsaw_local_y = source_jigsaw_pos.y - source_box_y;
            let mut base_height: Option<i32> = None;
            let Some(pool) = self.pool(&source_jigsaw.block.pool) else {
                continue;
            };
            if !self.pool_is_usable(pool) {
                continue;
            }
            let fallback = frozen.pools[pool.0 as usize].fallback;
            if !self.pool_is_usable(fallback) {
                continue;
            }
            let children_space = if source_bounds.is_inside(target_jigsaw_pos.into()) {
                *interior.get_or_insert_with(|| {
                    self.spaces.push(FreeSpace {
                        bounds: source_bounds,
                        holes: Vec::new(),
                    });
                    self.spaces.len() - 1
                })
            } else {
                space
            };

            let mut candidates = Vec::new();
            if depth != size {
                candidates.extend(shuffled(
                    &frozen.pools[pool.0 as usize].expanded,
                    &mut self.rng,
                ));
            }
            candidates.extend(shuffled(
                &frozen.pools[fallback.0 as usize].expanded,
                &mut self.rng,
            ));
            let placement_priority = source_jigsaw.block.placement_priority;

            'candidates: for candidate in candidates {
                let Some(target_projection) = frozen.elements[candidate.0 as usize].projection()
                else {
                    break;
                };
                let target_rigid = target_projection == Projection::Rigid;
                let mut rotations = Rotation::ALL;
                shuffle(&mut rotations, &mut self.rng);
                for target_rotation in rotations {
                    let target_jigsaws = shuffled_jigsaws(
                        frozen,
                        candidate,
                        IVec3::ZERO,
                        target_rotation,
                        &mut self.rng,
                    );
                    let expand_to = self.expansion_of(candidate, target_rotation, &target_jigsaws);
                    for target_jigsaw in &target_jigsaws {
                        if !can_attach(source_jigsaw, target_jigsaw) {
                            continue;
                        }
                        let target_jigsaw_local = target_jigsaw.pos;
                        let raw_target_box_pos = target_jigsaw_pos - target_jigsaw_local;
                        let raw_target_bounds =
                            element_bounds(frozen, candidate, raw_target_box_pos, target_rotation)
                                .expect("an empty element never reaches the candidate loop");
                        let target_jigsaw_local_y = target_jigsaw_local.y;
                        let delta_y =
                            source_jigsaw_local_y - target_jigsaw_local_y + front.normal().y;
                        let target_box_y = if source_rigid && target_rigid {
                            source_box_y + delta_y
                        } else {
                            self.surface_at(&mut base_height, source_jigsaw_pos)
                                - target_jigsaw_local_y
                        };
                        let lift = IVec3::new(0, target_box_y - raw_target_bounds.min.y, 0);
                        let mut target_bounds = raw_target_bounds.moved(lift);
                        let target_box_pos = raw_target_box_pos + lift;
                        if expand_to > 0 {
                            let new_size =
                                (expand_to + 1).max(target_bounds.max.y - target_bounds.min.y);
                            target_bounds = target_bounds.encapsulating(BlockPos::new(
                                target_bounds.min.x,
                                target_bounds.min.y + new_size,
                                target_bounds.min.z,
                            ));
                        }
                        if !self.spaces[children_space].fits(&target_bounds) {
                            continue;
                        }
                        self.spaces[children_space].holes.push(target_bounds);

                        let target_ground_level_delta = if target_rigid {
                            source_ground_level_delta - delta_y
                        } else {
                            1
                        };
                        let junction_y = if source_rigid {
                            source_box_y + source_jigsaw_local_y
                        } else if target_rigid {
                            target_box_y + target_jigsaw_local_y
                        } else {
                            self.surface_at(&mut base_height, source_jigsaw_pos) + delta_y / 2
                        };
                        self.pieces[source].junctions.push(Junction {
                            source_x: target_jigsaw_pos.x,
                            source_ground_y: junction_y - source_jigsaw_local_y
                                + source_ground_level_delta,
                            source_z: target_jigsaw_pos.z,
                            delta_y,
                            dest_projection: target_projection,
                        });
                        self.pieces.push(JigsawPiece {
                            element: candidate,
                            position: target_box_pos,
                            rotation: target_rotation,
                            bounds: target_bounds,
                            projection: target_projection,
                            ground_level_delta: target_ground_level_delta,
                            junctions: vec![Junction {
                                source_x: source_jigsaw_pos.x,
                                source_ground_y: junction_y - target_jigsaw_local_y
                                    + target_ground_level_delta,
                                source_z: source_jigsaw_pos.z,
                                delta_y: -delta_y,
                                dest_projection: source_projection,
                            }],
                        });
                        if depth < size {
                            self.queue.push(
                                placement_priority,
                                Pending {
                                    piece: self.pieces.len() - 1,
                                    space: children_space,
                                    depth: depth + 1,
                                },
                            );
                        }
                        break 'candidates;
                    }
                }
            }
        }
    }

    fn surface_at(&mut self, cached: &mut Option<i32>, jigsaw: IVec3) -> i32 {
        *cached.get_or_insert_with(|| {
            self.world
                .free_height(jigsaw.x, jigsaw.z, HeightmapName::WorldSurfaceWg)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cube(min: [i32; 3], max: [i32; 3]) -> BoundingBox {
        BoundingBox {
            min: min.into(),
            max: max.into(),
        }
    }

    #[test]
    fn a_candidate_touching_a_hole_on_one_face_is_rejected_and_one_block_over_fits() {
        let space = FreeSpace {
            bounds: cube([-50, 0, -50], [50, 100, 50]),
            holes: vec![cube([0, 0, 0], [9, 9, 9])],
        };
        assert!(!space.fits(&cube([9, 0, 0], [18, 9, 9])));
        assert!(space.fits(&cube([10, 0, 0], [19, 9, 9])));
        assert!(!space.fits(&cube([-9, -3, -9], [0, 0, 0])));
        assert!(space.fits(&cube([-9, 1, -9], [-1, 5, -1])));
    }

    #[test]
    fn a_candidate_crossing_the_context_boundary_is_rejected() {
        let interior = FreeSpace {
            bounds: cube([0, 0, 0], [15, 15, 15]),
            holes: Vec::new(),
        };
        assert!(interior.fits(&cube([0, 0, 0], [15, 15, 15])));
        assert!(!interior.fits(&cube([1, 1, 1], [16, 3, 3])));
        assert!(!interior.fits(&cube([-1, 1, 1], [3, 3, 3])));
    }

    #[test]
    fn the_queue_serves_the_highest_priority_first_and_fifo_within_it() {
        let mut queue = PriorityQueue::default();
        for (priority, piece) in [(0, 1), (5, 2), (0, 3), (5, 4), (-2, 5)] {
            queue.push(
                priority,
                Pending {
                    piece,
                    space: 0,
                    depth: 0,
                },
            );
        }
        let mut order = Vec::new();
        while let Some(pending) = queue.pop() {
            order.push(pending.piece);
        }
        assert_eq!(order, [2, 4, 1, 3, 5]);
    }
}
