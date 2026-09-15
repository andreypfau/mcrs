use bevy_math::IVec3;
use mcrs_minecraft_core::BoundingBox;
use mcrs_minecraft_random::Random;
use mcrs_minecraft_random::legacy::LegacyRandom;

use crate::orient::{Orientation, find_collision, move_inside_heights, orient_box, orient_box_at};
use crate::piece::{FortressKind, FortressPiece, Piece};
use crate::site::{Context, Site, Stub};

pub const SITE_IMPLIES_PIECE: Option<bool> = Some(true);

const MAX_DEPTH: i32 = 30;
const LOWEST_Y: i32 = 10;
const START_Y: i32 = 64;
const REACH: i32 = 112;

pub fn site(ctx: &mut Context<'_>, _rng: &mut LegacyRandom) -> Option<(IVec3, Stub)> {
    let chunk = ctx.chunk;
    Some((
        IVec3::new(chunk.min_block_x(), START_Y, chunk.min_block_z()),
        Stub::Plain,
    ))
}

#[derive(Clone, Copy)]
struct Weight {
    kind: FortressKind,
    weight: i32,
    max_place_count: i32,
    allow_in_row: bool,
}

const fn entry(
    kind: FortressKind,
    weight: i32,
    max_place_count: i32,
    allow_in_row: bool,
) -> Weight {
    Weight {
        kind,
        weight,
        max_place_count,
        allow_in_row,
    }
}

/// One of the reference's two piece tables: its weights, how often each
/// entry has been placed, and which entries are still offered.
struct Table {
    weights: Vec<Weight>,
    place_count: Vec<i32>,
    available: Vec<usize>,
}

impl Table {
    fn new(weights: &[Weight]) -> Self {
        Table {
            weights: weights.to_vec(),
            place_count: vec![0; weights.len()],
            available: (0..weights.len()).collect(),
        }
    }

    fn valid(&self, entry: usize) -> bool {
        let max = self.weights[entry].max_place_count;
        max == 0 || self.place_count[entry] < max
    }

    /// `updatePieceWeight`: the offered weight, or `-1` when no entry with a
    /// cap is under it.
    fn total_weight(&self) -> i32 {
        let mut any = false;
        let mut total = 0;
        for &entry in &self.available {
            let weight = &self.weights[entry];
            if weight.max_place_count > 0 && self.place_count[entry] < weight.max_place_count {
                any = true;
            }
            total += weight.weight;
        }
        if any { total } else { -1 }
    }
}

#[derive(Clone, Copy)]
enum Side {
    Forward,
    Left,
    Right,
}

struct Layout {
    pieces: Vec<Piece>,
    pending: Vec<usize>,
    bridge: Table,
    castle: Table,
    previous: Option<(bool, usize)>,
    start_min: IVec3,
}

pub fn layout(ctx: &mut Context<'_>, mut site: Site) -> Vec<Piece> {
    use FortressKind::*;
    let bridge_weights = [
        entry(BridgeStraight, 30, 0, true),
        entry(BridgeCrossing, 10, 4, false),
        entry(RoomCrossing, 10, 4, false),
        entry(StairsRoom, 10, 3, false),
        entry(MonsterThrone, 5, 2, false),
        entry(CastleEntrance, 5, 1, false),
    ];
    let castle_weights = [
        entry(SmallCorridor, 25, 0, true),
        entry(SmallCorridorCrossing, 15, 5, false),
        entry(SmallCorridorRightTurn { chest: false }, 5, 10, false),
        entry(SmallCorridorLeftTurn { chest: false }, 5, 10, false),
        entry(CorridorStairs, 10, 3, true),
        entry(CorridorBalcony, 7, 2, false),
        entry(StalkRoom, 5, 2, false),
    ];
    let rng = &mut site.rng;
    let origin = IVec3::new(
        ctx.chunk.min_block_x() + 2,
        START_Y,
        ctx.chunk.min_block_z() + 2,
    );
    let orientation = Orientation::random(rng);
    let start = FortressPiece {
        kind: BridgeCrossing,
        bounds: orient_box(orientation, origin, 19, 10, 19),
        orientation,
        gen_depth: 0,
    };
    let mut layout = Layout {
        start_min: *start.bounds.min,
        pieces: vec![Piece::Fortress(start)],
        pending: Vec::new(),
        bridge: Table::new(&bridge_weights),
        castle: Table::new(&castle_weights),
        previous: None,
    };
    layout.add_children(0, rng);
    while !layout.pending.is_empty() {
        let index = rng.next_i32_bound(layout.pending.len() as i32) as usize;
        let piece = layout.pending.remove(index);
        layout.add_children(piece, rng);
    }
    move_inside_heights(&mut layout.pieces, rng, 48, 70);
    layout.pieces
}

impl Layout {
    fn table(&self, castle: bool) -> &Table {
        if castle { &self.castle } else { &self.bridge }
    }

    fn piece(&self, index: usize) -> &FortressPiece {
        match &self.pieces[index] {
            Piece::Fortress(piece) => piece,
            _ => unreachable!("a fortress lays out fortress pieces only"),
        }
    }

    /// Each type's `addChildren`.
    fn add_children(&mut self, index: usize, rng: &mut LegacyRandom) {
        use FortressKind::*;
        use Side::*;
        let piece = self.piece(index);
        let orientation = piece.orientation;
        match piece.kind {
            BridgeCrossing => {
                self.child(index, rng, Forward, 8, 3, false);
                self.child(index, rng, Left, 3, 8, false);
                self.child(index, rng, Right, 3, 8, false);
            }
            BridgeStraight => self.child(index, rng, Forward, 1, 3, false),
            RoomCrossing => {
                self.child(index, rng, Forward, 2, 0, false);
                self.child(index, rng, Left, 0, 2, false);
                self.child(index, rng, Right, 0, 2, false);
            }
            StairsRoom => self.child(index, rng, Right, 6, 2, false),
            CastleEntrance => self.child(index, rng, Forward, 5, 3, true),
            SmallCorridor | CorridorStairs => self.child(index, rng, Forward, 1, 0, true),
            SmallCorridorCrossing => {
                self.child(index, rng, Forward, 1, 0, true);
                self.child(index, rng, Left, 0, 1, true);
                self.child(index, rng, Right, 0, 1, true);
            }
            SmallCorridorRightTurn { .. } => self.child(index, rng, Right, 0, 1, true),
            SmallCorridorLeftTurn { .. } => self.child(index, rng, Left, 0, 1, true),
            CorridorBalcony => {
                let offset = match orientation {
                    Orientation::West | Orientation::North => 5,
                    Orientation::South | Orientation::East => 1,
                };
                let castle = rng.next_i32_bound(8) > 0;
                self.child(index, rng, Left, 0, offset, castle);
                let castle = rng.next_i32_bound(8) > 0;
                self.child(index, rng, Right, 0, offset, castle);
            }
            StalkRoom => {
                self.child(index, rng, Forward, 5, 3, true);
                self.child(index, rng, Forward, 5, 11, true);
            }
            BridgeEndFiller { .. } | MonsterThrone => {}
        }
    }

    /// `generateChildForward`, `generateChildLeft` and `generateChildRight`:
    /// `a` is the forward child's x offset or the side child's y offset, `b`
    /// the forward child's y offset or the side child's z offset.
    fn child(
        &mut self,
        parent: usize,
        rng: &mut LegacyRandom,
        side: Side,
        a: i32,
        b: i32,
        castle: bool,
    ) {
        let piece = self.piece(parent);
        let (min, max) = (*piece.bounds.min, *piece.bounds.max);
        let facing = piece.orientation;
        let depth = piece.gen_depth;
        let (foot, direction) = match (side, facing) {
            (Side::Forward, Orientation::North) => {
                (IVec3::new(min.x + a, min.y + b, min.z - 1), facing)
            }
            (Side::Forward, Orientation::South) => {
                (IVec3::new(min.x + a, min.y + b, max.z + 1), facing)
            }
            (Side::Forward, Orientation::West) => {
                (IVec3::new(min.x - 1, min.y + b, min.z + a), facing)
            }
            (Side::Forward, Orientation::East) => {
                (IVec3::new(max.x + 1, min.y + b, min.z + a), facing)
            }
            (Side::Left, Orientation::North | Orientation::South) => (
                IVec3::new(min.x - 1, min.y + a, min.z + b),
                Orientation::West,
            ),
            (Side::Left, Orientation::West | Orientation::East) => (
                IVec3::new(min.x + b, min.y + a, min.z - 1),
                Orientation::North,
            ),
            (Side::Right, Orientation::North | Orientation::South) => (
                IVec3::new(max.x + 1, min.y + a, min.z + b),
                Orientation::East,
            ),
            (Side::Right, Orientation::West | Orientation::East) => (
                IVec3::new(min.x + b, min.y + a, max.z + 1),
                Orientation::South,
            ),
        };
        self.generate_and_add(rng, foot, direction, depth, castle);
    }

    /// `generateAndAddPiece`: past the reach an end filler is built and
    /// dropped, spending its seed draw.
    fn generate_and_add(
        &mut self,
        rng: &mut LegacyRandom,
        foot: IVec3,
        direction: Orientation,
        depth: i32,
        castle: bool,
    ) {
        if (foot.x - self.start_min.x).abs() > REACH || (foot.z - self.start_min.z).abs() > REACH {
            self.end_filler(rng, foot, direction, depth);
            return;
        }
        if let Some(piece) = self.generate_piece(rng, foot, direction, depth + 1, castle) {
            self.pending.push(self.pieces.len());
            self.pieces.push(Piece::Fortress(piece));
        }
    }

    /// `generatePiece`: five weighted draws, each walking the table from the
    /// drawn entry on until one builds, then the end filler.
    fn generate_piece(
        &mut self,
        rng: &mut LegacyRandom,
        foot: IVec3,
        direction: Orientation,
        depth: i32,
        castle: bool,
    ) -> Option<FortressPiece> {
        let total = self.table(castle).total_weight();
        if total > 0 && depth <= MAX_DEPTH {
            for _ in 0..5 {
                let mut selection = rng.next_i32_bound(total);
                let table = self.table(castle);
                for position in 0..table.available.len() {
                    let entry = table.available[position];
                    let weight = table.weights[entry];
                    selection -= weight.weight;
                    if selection >= 0 {
                        continue;
                    }
                    let previous = self.previous == Some((castle, entry));
                    if !table.valid(entry) || (previous && !weight.allow_in_row) {
                        break;
                    }
                    let Some(piece) = self.create(rng, weight.kind, foot, direction, depth) else {
                        continue;
                    };
                    let table = if castle {
                        &mut self.castle
                    } else {
                        &mut self.bridge
                    };
                    table.place_count[entry] += 1;
                    self.previous = Some((castle, entry));
                    if !table.valid(entry) {
                        table.available.remove(position);
                    }
                    return Some(piece);
                }
            }
        }
        self.end_filler(rng, foot, direction, depth)
    }

    fn end_filler(
        &self,
        rng: &mut LegacyRandom,
        foot: IVec3,
        direction: Orientation,
        depth: i32,
    ) -> Option<FortressPiece> {
        self.create(
            rng,
            FortressKind::BridgeEndFiller { seed: 0 },
            foot,
            direction,
            depth,
        )
    }

    /// Each type's `createPiece`: the box, its floor above 10, no collision,
    /// then the constructor's own draw.
    fn create(
        &self,
        rng: &mut LegacyRandom,
        kind: FortressKind,
        foot: IVec3,
        direction: Orientation,
        depth: i32,
    ) -> Option<FortressPiece> {
        use FortressKind::*;
        let (offset, size) = match kind {
            BridgeCrossing => ([-8, -3, 0], [19, 10, 19]),
            BridgeEndFiller { .. } => ([-1, -3, 0], [5, 10, 8]),
            BridgeStraight => ([-1, -3, 0], [5, 10, 19]),
            CorridorStairs => ([-1, -7, 0], [5, 14, 10]),
            CorridorBalcony => ([-3, 0, 0], [9, 7, 9]),
            CastleEntrance | StalkRoom => ([-5, -3, 0], [13, 14, 13]),
            SmallCorridorCrossing
            | SmallCorridorLeftTurn { .. }
            | SmallCorridor
            | SmallCorridorRightTurn { .. } => ([-1, 0, 0], [5, 7, 5]),
            MonsterThrone => ([-2, 0, 0], [7, 8, 9]),
            RoomCrossing => ([-2, 0, 0], [7, 9, 7]),
            StairsRoom => ([-2, 0, 0], [7, 11, 7]),
        };
        let [width, height, depth_z] = size;
        let bounds = orient_box_at(direction, foot, offset.into(), width, height, depth_z);
        if !ok_box(bounds) || find_collision(&self.pieces, bounds).is_some() {
            return None;
        }
        let kind = match kind {
            BridgeEndFiller { .. } => BridgeEndFiller {
                seed: rng.next_i32(),
            },
            SmallCorridorLeftTurn { .. } => SmallCorridorLeftTurn {
                chest: rng.next_i32_bound(3) == 0,
            },
            SmallCorridorRightTurn { .. } => SmallCorridorRightTurn {
                chest: rng.next_i32_bound(3) == 0,
            },
            other => other,
        };
        Some(FortressPiece {
            kind,
            bounds,
            orientation: direction,
            gen_depth: depth,
        })
    }
}

fn ok_box(bounds: BoundingBox) -> bool {
    bounds.min.y > LOWEST_Y
}
