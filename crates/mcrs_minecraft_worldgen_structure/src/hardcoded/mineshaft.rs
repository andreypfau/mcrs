use std::cmp::Ordering;

use bevy_math::IVec3;
use mcrs_minecraft_core::{BlockPos, BoundingBox, ColumnPos};
use mcrs_minecraft_random::Random;
use mcrs_minecraft_random::legacy::LegacyRandom;
use mcrs_minecraft_worldgen_feature::placement::HeightmapName;

use crate::MineshaftType;
use crate::orient::{
    Orientation, find_collision, move_below_sea_level, offset_vertically, world_pos,
};
use crate::piece::{MineshaftKind, MineshaftPiece, Piece};
use crate::site::{Context, Site, SiteWorld, Stub};

const MAX_DEPTH: i32 = 8;
const REACH: i32 = 80;
const START_Y: i32 = 50;

/// `findGenerationPoint`: the whole piece tree is built here, before the
/// biome test, so its draws happen whether or not the site passes.
pub fn site(
    mineshaft_type: MineshaftType,
    ctx: &mut Context<'_>,
    rng: &mut LegacyRandom,
) -> Option<(IVec3, Stub)> {
    rng.next_f64();
    let chunk = ctx.chunk;
    let start = IVec3::new(chunk.middle_block_x(), START_Y, chunk.min_block_z());
    let mut pieces = piece_tree(rng, chunk.min_block_x() + 2, chunk.min_block_z() + 2);
    let sea_level = ctx.height.sea_level;
    let dy = match mineshaft_type {
        MineshaftType::Mesa => {
            let bounds = pieces
                .iter()
                .map(Piece::bounds)
                .reduce(BoundingBox::union)
                .expect("the room");
            let centre = bounds.center();
            let surface = ctx
                .world
                .free_height(centre.x, centre.z, HeightmapName::WorldSurfaceWg);
            let target = if surface <= sea_level {
                sea_level
            } else {
                rng.next_int_between_inclusive(sea_level, surface)
            };
            let dy = target - centre.y;
            offset_vertically(&mut pieces, dy);
            dy
        }
        MineshaftType::Normal => {
            move_below_sea_level(&mut pieces, sea_level, ctx.height.min_y, rng, 10)
        }
    };
    Some((start + IVec3::new(0, dy, 0), Stub::Mineshaft(pieces)))
}

pub fn layout(_mineshaft_type: MineshaftType, ctx: &mut Context<'_>, site: Site) -> Vec<Piece> {
    let Stub::Mineshaft(mut pieces) = site.stub else {
        unreachable!("a mineshaft site carries its pieces")
    };
    for piece in &mut pieces {
        let Piece::Mineshaft(MineshaftPiece {
            kind:
                MineshaftKind::Corridor {
                    spider_corridor: true,
                    num_sections,
                    spawner_host,
                    ..
                },
            bounds,
            direction,
            ..
        }) = piece
        else {
            continue;
        };
        *spawner_host = spawner_host_of(*bounds, *direction, *num_sections, ctx.world);
    }
    pieces
}

/// Which of two columns a corridor crossing both decorates first. The
/// reference leaves this to the order chunks load; players and pregenerators
/// both walk outward from the origin, so the column nearer to it goes first.
pub fn decorates_first(a: ColumnPos, b: ColumnPos) -> Ordering {
    let distance = |column: ColumnPos| {
        let (x, z) = (i64::from(column.x) * 16 + 8, i64::from(column.z) * 16 + 8);
        x * x + z * z
    };
    distance(a)
        .cmp(&distance(b))
        .then(a.x.cmp(&b.x))
        .then(a.z.cmp(&b.z))
}

/// The reference draws the spawner's cell per section from the placement
/// stream of whichever chunk decorates the corridor first, and the chunks
/// after it never draw. The column that decorates first among those holding a
/// section whose three candidate cells all lie inside it and under the ocean
/// floor stands in for that chunk: the reference's draw there cannot miss, so
/// it always places by that section. Every corridor of two sections or more
/// has such a section, since chunk borders can cut at most one of its windows.
fn spawner_host_of(
    bounds: BoundingBox,
    direction: Option<Orientation>,
    num_sections: i32,
    world: &mut dyn SiteWorld,
) -> Option<ColumnPos> {
    let mut host: Option<ColumnPos> = None;
    for section in 0..num_sections {
        let z = 2 + section * 5;
        let cells = [-1, 0, 1].map(|dz| world_pos(direction, bounds, IVec3::new(1, 0, z + dz)));
        let column = ColumnPos::new(cells[0].x >> 4, cells[0].z >> 4);
        if host.is_some_and(|host| decorates_first(column, host) != Ordering::Less) {
            continue;
        }
        let safe = cells.iter().all(|cell| {
            ColumnPos::new(cell.x >> 4, cell.z >> 4) == column
                && cell.y + 1 < world.free_height(cell.x, cell.z, HeightmapName::OceanFloorWg)
        });
        if safe {
            host = Some(column);
        }
    }
    host
}

fn piece_tree(rng: &mut LegacyRandom, west: i32, north: i32) -> Vec<Piece> {
    let max_x = west + 7 + rng.next_i32_bound(6);
    let max_y = 54 + rng.next_i32_bound(6);
    let max_z = north + 7 + rng.next_i32_bound(6);
    let room = MineshaftPiece {
        kind: MineshaftKind::Room {
            entrances: Vec::new(),
        },
        bounds: BoundingBox {
            min: BlockPos::new(west, START_Y, north),
            max: BlockPos::new(max_x, max_y, max_z),
        },
        direction: None,
        gen_depth: 0,
    };
    let mut layout = Layout {
        pieces: vec![Piece::Mineshaft(room)],
    };
    layout.room_children(rng);
    layout.pieces
}

struct Layout {
    pieces: Vec<Piece>,
}

impl Layout {
    fn piece(&self, index: usize) -> &MineshaftPiece {
        match &self.pieces[index] {
            Piece::Mineshaft(piece) => piece,
            _ => unreachable!("a mineshaft lays out mineshaft pieces only"),
        }
    }

    /// `MineShaftRoom.addChildren`: a walk along each wall, every child's
    /// entrance box recorded on the room.
    fn room_children(&mut self, rng: &mut LegacyRandom) {
        use Orientation::*;
        let bounds = self.piece(0).bounds;
        let (min, max) = (*bounds.min, *bounds.max);
        let height_space = (bounds.y_span() - 3 - 1).max(1);
        let x_span = max.x - min.x + 1;
        let z_span = max.z - min.z + 1;
        let walls = [
            (x_span, North),
            (x_span, South),
            (z_span, West),
            (z_span, East),
        ];
        let mut entrances = Vec::new();
        for (span, direction) in walls {
            let mut pos = 0;
            while pos < span {
                pos += rng.next_i32_bound(span);
                if pos + 3 > span {
                    break;
                }
                let y = min.y + rng.next_i32_bound(height_space) + 1;
                let foot = match direction {
                    North => IVec3::new(min.x + pos, y, min.z - 1),
                    South => IVec3::new(min.x + pos, y, max.z + 1),
                    West => IVec3::new(min.x - 1, y, min.z + pos),
                    East => IVec3::new(max.x + 1, y, min.z + pos),
                };
                if let Some(child) = self.generate_and_add(rng, foot, direction, 0) {
                    let child = self.piece(child).bounds;
                    let (cmin, cmax) = (*child.min, *child.max);
                    let (entrance_min, entrance_max) = match direction {
                        North => (
                            IVec3::new(cmin.x, cmin.y, min.z),
                            IVec3::new(cmax.x, cmax.y, min.z + 1),
                        ),
                        South => (
                            IVec3::new(cmin.x, cmin.y, max.z - 1),
                            IVec3::new(cmax.x, cmax.y, max.z),
                        ),
                        West => (
                            IVec3::new(min.x, cmin.y, cmin.z),
                            IVec3::new(min.x + 1, cmax.y, cmax.z),
                        ),
                        East => (
                            IVec3::new(max.x - 1, cmin.y, cmin.z),
                            IVec3::new(max.x, cmax.y, cmax.z),
                        ),
                    };
                    entrances.push(BoundingBox {
                        min: entrance_min.into(),
                        max: entrance_max.into(),
                    });
                }
                pos += 4;
            }
        }
        if let Piece::Mineshaft(MineshaftPiece {
            kind: MineshaftKind::Room { entrances: held },
            ..
        }) = &mut self.pieces[0]
        {
            *held = entrances;
        }
    }

    /// `generateAndAddPiece`: within the depth and the reach of the room, a
    /// piece is built and its children added before the caller continues.
    fn generate_and_add(
        &mut self,
        rng: &mut LegacyRandom,
        foot: IVec3,
        direction: Orientation,
        depth: i32,
    ) -> Option<usize> {
        if depth > MAX_DEPTH {
            return None;
        }
        let start = self.piece(0).bounds.min;
        if (foot.x - start.x).abs() > REACH || (foot.z - start.z).abs() > REACH {
            return None;
        }
        let piece = self.create_random(rng, foot, direction, depth + 1)?;
        let index = self.pieces.len();
        self.pieces.push(Piece::Mineshaft(piece));
        self.add_children(index, rng);
        Some(index)
    }

    /// `createRandomShaftPiece`.
    fn create_random(
        &self,
        rng: &mut LegacyRandom,
        foot: IVec3,
        direction: Orientation,
        gen_depth: i32,
    ) -> Option<MineshaftPiece> {
        use Orientation::*;
        let selection = rng.next_i32_bound(100);
        let local = |min: [i32; 3], max: [i32; 3]| BoundingBox {
            min: (IVec3::from(min) + foot).into(),
            max: (IVec3::from(max) + foot).into(),
        };
        if selection >= 80 {
            let y1 = if rng.next_i32_bound(4) == 0 { 6 } else { 2 };
            let bounds = match direction {
                North => local([-1, 0, -4], [3, y1, 0]),
                South => local([-1, 0, 0], [3, y1, 4]),
                West => local([-4, 0, -1], [0, y1, 3]),
                East => local([0, 0, -1], [4, y1, 3]),
            };
            if find_collision(&self.pieces, bounds).is_some() {
                return None;
            }
            Some(MineshaftPiece {
                kind: MineshaftKind::Crossing {
                    two_floored: bounds.y_span() > 3,
                },
                bounds,
                direction: Some(direction),
                gen_depth,
            })
        } else if selection >= 70 {
            let bounds = match direction {
                North => local([0, -5, -8], [2, 2, 0]),
                South => local([0, -5, 0], [2, 2, 8]),
                West => local([-8, -5, 0], [0, 2, 2]),
                East => local([0, -5, 0], [8, 2, 2]),
            };
            if find_collision(&self.pieces, bounds).is_some() {
                return None;
            }
            Some(MineshaftPiece {
                kind: MineshaftKind::Stairs,
                bounds,
                direction: Some(direction),
                gen_depth,
            })
        } else {
            let mut length = rng.next_i32_bound(3) + 2;
            let mut found = None;
            while length > 0 {
                let blocks = length * 5;
                let bounds = match direction {
                    North => local([0, 0, -(blocks - 1)], [2, 2, 0]),
                    South => local([0, 0, 0], [2, 2, blocks - 1]),
                    West => local([-(blocks - 1), 0, 0], [0, 2, 2]),
                    East => local([0, 0, 0], [blocks - 1, 2, 2]),
                };
                if find_collision(&self.pieces, bounds).is_none() {
                    found = Some(bounds);
                    break;
                }
                length -= 1;
            }
            let bounds = found?;
            let has_rails = rng.next_i32_bound(3) == 0;
            let spider_corridor = !has_rails && rng.next_i32_bound(23) == 0;
            let num_sections = match direction {
                North | South => (bounds.max.z - bounds.min.z + 1) / 5,
                West | East => (bounds.max.x - bounds.min.x + 1) / 5,
            };
            Some(MineshaftPiece {
                kind: MineshaftKind::Corridor {
                    has_rails,
                    spider_corridor,
                    num_sections,
                    spawner_host: None,
                },
                bounds,
                direction: Some(direction),
                gen_depth,
            })
        }
    }

    /// Each type's `addChildren`.
    fn add_children(&mut self, index: usize, rng: &mut LegacyRandom) {
        use Orientation::*;
        let piece = self.piece(index);
        let bounds = piece.bounds;
        let (min, max) = (*bounds.min, *bounds.max);
        let depth = piece.gen_depth;
        let facing = piece.direction.expect("every child piece faces somewhere");
        match piece.kind.clone() {
            MineshaftKind::Room { .. } => unreachable!("the room is the start"),
            MineshaftKind::Corridor { .. } => {
                let end = rng.next_i32_bound(4);
                let y = |rng: &mut LegacyRandom| min.y - 1 + rng.next_i32_bound(3);
                let (foot, direction) = match (facing, end) {
                    (North, 0 | 1) => (IVec3::new(min.x, y(rng), min.z - 1), North),
                    (North, 2) => (IVec3::new(min.x - 1, y(rng), min.z), West),
                    (North, _) => (IVec3::new(max.x + 1, y(rng), min.z), East),
                    (South, 0 | 1) => (IVec3::new(min.x, y(rng), max.z + 1), South),
                    (South, 2) => (IVec3::new(min.x - 1, y(rng), max.z - 3), West),
                    (South, _) => (IVec3::new(max.x + 1, y(rng), max.z - 3), East),
                    (West, 0 | 1) => (IVec3::new(min.x - 1, y(rng), min.z), West),
                    (West, 2) => (IVec3::new(min.x, y(rng), min.z - 1), North),
                    (West, _) => (IVec3::new(min.x, y(rng), max.z + 1), South),
                    (East, 0 | 1) => (IVec3::new(max.x + 1, y(rng), min.z), East),
                    (East, 2) => (IVec3::new(max.x - 3, y(rng), min.z - 1), North),
                    (East, _) => (IVec3::new(max.x - 3, y(rng), max.z + 1), South),
                };
                self.generate_and_add(rng, foot, direction, depth);
                if depth < MAX_DEPTH {
                    match facing {
                        West | East => {
                            let mut x = min.x + 3;
                            while x + 3 <= max.x {
                                match rng.next_i32_bound(5) {
                                    0 => {
                                        let foot = IVec3::new(x, min.y, min.z - 1);
                                        self.generate_and_add(rng, foot, North, depth + 1);
                                    }
                                    1 => {
                                        let foot = IVec3::new(x, min.y, max.z + 1);
                                        self.generate_and_add(rng, foot, South, depth + 1);
                                    }
                                    _ => {}
                                }
                                x += 5;
                            }
                        }
                        North | South => {
                            let mut z = min.z + 3;
                            while z + 3 <= max.z {
                                match rng.next_i32_bound(5) {
                                    0 => {
                                        let foot = IVec3::new(min.x - 1, min.y, z);
                                        self.generate_and_add(rng, foot, West, depth + 1);
                                    }
                                    1 => {
                                        let foot = IVec3::new(max.x + 1, min.y, z);
                                        self.generate_and_add(rng, foot, East, depth + 1);
                                    }
                                    _ => {}
                                }
                                z += 5;
                            }
                        }
                    }
                }
            }
            MineshaftKind::Crossing { two_floored } => {
                let north = |y| (IVec3::new(min.x + 1, y, min.z - 1), North);
                let south = |y| (IVec3::new(min.x + 1, y, max.z + 1), South);
                let west = |y| (IVec3::new(min.x - 1, y, min.z + 1), West);
                let east = |y| (IVec3::new(max.x + 1, y, min.z + 1), East);
                let arms = match facing {
                    North => [north(min.y), west(min.y), east(min.y)],
                    South => [south(min.y), west(min.y), east(min.y)],
                    West => [north(min.y), south(min.y), west(min.y)],
                    East => [north(min.y), south(min.y), east(min.y)],
                };
                for (foot, direction) in arms {
                    self.generate_and_add(rng, foot, direction, depth);
                }
                if two_floored {
                    let upper = min.y + 3 + 1;
                    for (foot, direction) in [north(upper), west(upper), east(upper), south(upper)]
                    {
                        if rng.next_bool() {
                            self.generate_and_add(rng, foot, direction, depth);
                        }
                    }
                }
            }
            MineshaftKind::Stairs => {
                let foot = match facing {
                    North => IVec3::new(min.x, min.y, min.z - 1),
                    South => IVec3::new(min.x, min.y, max.z + 1),
                    West => IVec3::new(min.x - 1, min.y, min.z),
                    East => IVec3::new(max.x + 1, min.y, min.z),
                };
                self.generate_and_add(rng, foot, facing, depth);
            }
        }
    }
}
