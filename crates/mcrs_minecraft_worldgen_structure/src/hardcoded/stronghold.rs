use bevy_math::IVec3;
use mcrs_minecraft_core::BoundingBox;
use mcrs_minecraft_random::Random;
use mcrs_minecraft_random::legacy::LegacyRandom;

use crate::orient::{Orientation, find_collision, move_below_sea_level, orient_box, orient_box_at};
use crate::piece::{Piece, SmallDoor, StrongholdKind, StrongholdPiece};
use crate::site::{Context, Site, Stub};

pub const SITE_IMPLIES_PIECE: Option<bool> = Some(true);

const MAX_DEPTH: i32 = 50;
const LOWEST_Y: i32 = 10;
const START_Y: i32 = 64;
const REACH: i32 = 112;
const SEA_LEVEL_OFFSET: i32 = 10;

pub fn site(ctx: &mut Context<'_>, _rng: &mut LegacyRandom) -> Option<(IVec3, Stub)> {
    let chunk = ctx.chunk;
    Some((
        IVec3::new(chunk.min_block_x(), 0, chunk.min_block_z()),
        Stub::Plain,
    ))
}

#[derive(Clone, Copy)]
struct Weight {
    kind: StrongholdKind,
    weight: i32,
    max_place_count: i32,
    /// The generation depth a piece must exceed to be offered.
    min_depth: i32,
}

const fn entry(kind: StrongholdKind, weight: i32, max_place_count: i32, min_depth: i32) -> Weight {
    Weight {
        kind,
        weight,
        max_place_count,
        min_depth,
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
    weights: Vec<Weight>,
    place_count: Vec<i32>,
    available: Vec<usize>,
    previous: Option<usize>,
    imposed: Option<StrongholdKind>,
    portal_room: bool,
    start_min: IVec3,
}

/// `StrongholdStructure.generatePieces`: the whole tree is rebuilt on the
/// chunk's large-feature stream salted by the try count until a portal room
/// is part of it.
pub fn layout(ctx: &mut Context<'_>, _site: Site) -> Vec<Piece> {
    use StrongholdKind::*;
    let weights = [
        entry(
            Straight {
                left: false,
                right: false,
            },
            40,
            0,
            0,
        ),
        entry(PrisonHall, 5, 5, 0),
        entry(LeftTurn, 20, 0, 0),
        entry(RightTurn, 20, 0, 0),
        entry(RoomCrossing { variant: 0 }, 10, 6, 0),
        entry(StraightStairsDown, 5, 5, 0),
        entry(StairsDown, 5, 5, 0),
        entry(
            FiveCrossing {
                left_low: false,
                left_high: false,
                right_low: false,
                right_high: false,
            },
            5,
            4,
            0,
        ),
        entry(ChestCorridor, 5, 4, 0),
        entry(Library { tall: false }, 10, 2, 4),
        entry(PortalRoom, 20, 1, 5),
    ];
    let chunk = ctx.chunk;
    let origin = IVec3::new(chunk.min_block_x() + 2, START_Y, chunk.min_block_z() + 2);
    let mut tries = 0i64;
    loop {
        let rng = &mut LegacyRandom::large_feature(ctx.seed.wrapping_add(tries), chunk.x, chunk.z);
        tries += 1;
        let orientation = Orientation::random(rng);
        let start = StrongholdPiece {
            kind: Start,
            entry_door: SmallDoor::Opening,
            bounds: orient_box(orientation, origin, 5, 11, 5),
            orientation,
            gen_depth: 0,
        };
        let mut layout = Layout {
            start_min: *start.bounds.min,
            pieces: vec![Piece::Stronghold(start)],
            pending: Vec::new(),
            weights: weights.to_vec(),
            place_count: vec![0; weights.len()],
            available: (0..weights.len()).collect(),
            previous: None,
            imposed: None,
            portal_room: false,
        };
        layout.add_children(0, rng);
        while !layout.pending.is_empty() {
            let index = rng.next_i32_bound(layout.pending.len() as i32) as usize;
            let piece = layout.pending.remove(index);
            layout.add_children(piece, rng);
        }
        move_below_sea_level(
            &mut layout.pieces,
            ctx.height.sea_level,
            ctx.height.min_y,
            rng,
            SEA_LEVEL_OFFSET,
        );
        if layout.portal_room {
            return layout.pieces;
        }
    }
}

impl Layout {
    fn piece(&self, index: usize) -> &StrongholdPiece {
        match &self.pieces[index] {
            Piece::Stronghold(piece) => piece,
            _ => unreachable!("a stronghold lays out stronghold pieces only"),
        }
    }

    fn valid(&self, entry: usize) -> bool {
        let max = self.weights[entry].max_place_count;
        max == 0 || self.place_count[entry] < max
    }

    fn do_place(&self, entry: usize, depth: i32) -> bool {
        self.valid(entry) && depth > self.weights[entry].min_depth
    }

    /// `updatePieceWeight`: the offered weight, or `None` when no entry with
    /// a cap is under it.
    fn total_weight(&self) -> Option<i32> {
        let mut any = false;
        let mut total = 0;
        for &entry in &self.available {
            let weight = &self.weights[entry];
            if weight.max_place_count > 0 && self.place_count[entry] < weight.max_place_count {
                any = true;
            }
            total += weight.weight;
        }
        any.then_some(total)
    }

    /// Each type's `addChildren`.
    fn add_children(&mut self, index: usize, rng: &mut LegacyRandom) {
        use Side::*;
        use StrongholdKind::*;
        let piece = self.piece(index);
        let orientation = piece.orientation;
        let turns_left = matches!(orientation, Orientation::North | Orientation::East);
        match piece.kind {
            Start => {
                self.imposed = Some(FiveCrossing {
                    left_low: false,
                    left_high: false,
                    right_low: false,
                    right_high: false,
                });
                self.child(index, rng, Forward, 1, 1);
            }
            StairsDown | PrisonHall | StraightStairsDown | ChestCorridor => {
                self.child(index, rng, Forward, 1, 1);
            }
            Straight { left, right } => {
                self.child(index, rng, Forward, 1, 1);
                if left {
                    self.child(index, rng, Left, 1, 2);
                }
                if right {
                    self.child(index, rng, Right, 1, 2);
                }
            }
            LeftTurn => {
                let side = if turns_left { Left } else { Right };
                self.child(index, rng, side, 1, 1);
            }
            RightTurn => {
                let side = if turns_left { Right } else { Left };
                self.child(index, rng, side, 1, 1);
            }
            RoomCrossing { .. } => {
                self.child(index, rng, Forward, 4, 1);
                self.child(index, rng, Left, 1, 4);
                self.child(index, rng, Right, 1, 4);
            }
            FiveCrossing {
                left_low,
                left_high,
                right_low,
                right_high,
            } => {
                let (low, high) = match orientation {
                    Orientation::West | Orientation::North => (5, 3),
                    Orientation::South | Orientation::East => (3, 5),
                };
                self.child(index, rng, Forward, 5, 1);
                if left_low {
                    self.child(index, rng, Left, low, 1);
                }
                if left_high {
                    self.child(index, rng, Left, high, 7);
                }
                if right_low {
                    self.child(index, rng, Right, low, 1);
                }
                if right_high {
                    self.child(index, rng, Right, high, 7);
                }
            }
            PortalRoom => self.portal_room = true,
            Library { .. } | FillerCorridor { .. } => {}
        }
    }

    /// `generateSmallDoorChildForward`, `generateSmallDoorChildLeft` and
    /// `generateSmallDoorChildRight`: `a` is the forward child's x offset or
    /// the side child's y offset, `b` the forward child's y offset or the side
    /// child's z offset.
    fn child(&mut self, parent: usize, rng: &mut LegacyRandom, side: Side, a: i32, b: i32) {
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
        self.generate_and_add(rng, foot, direction, depth);
    }

    /// `generateAndAddPiece`.
    fn generate_and_add(
        &mut self,
        rng: &mut LegacyRandom,
        foot: IVec3,
        direction: Orientation,
        depth: i32,
    ) {
        if depth > MAX_DEPTH
            || (foot.x - self.start_min.x).abs() > REACH
            || (foot.z - self.start_min.z).abs() > REACH
        {
            return;
        }
        if let Some(piece) = self.generate_piece(rng, foot, direction, depth + 1) {
            self.pending.push(self.pieces.len());
            self.pieces.push(Piece::Stronghold(piece));
        }
    }

    /// `generatePieceFromSmallDoor`: the imposed piece first, then five
    /// weighted draws each walking the table from the drawn entry on until
    /// one builds, then a filler corridor up to whatever is in the way.
    fn generate_piece(
        &mut self,
        rng: &mut LegacyRandom,
        foot: IVec3,
        direction: Orientation,
        depth: i32,
    ) -> Option<StrongholdPiece> {
        let total = self.total_weight()?;
        if let Some(kind) = self.imposed.take()
            && let Some(piece) = self.create(rng, kind, foot, direction, depth)
        {
            return Some(piece);
        }
        for _ in 0..5 {
            let mut selection = rng.next_i32_bound(total);
            for position in 0..self.available.len() {
                let entry = self.available[position];
                let weight = self.weights[entry];
                selection -= weight.weight;
                if selection >= 0 {
                    continue;
                }
                if !self.do_place(entry, depth) || self.previous == Some(entry) {
                    break;
                }
                let Some(piece) = self.create(rng, weight.kind, foot, direction, depth) else {
                    continue;
                };
                self.place_count[entry] += 1;
                self.previous = Some(entry);
                if !self.valid(entry) {
                    self.available.remove(position);
                }
                return Some(piece);
            }
        }
        self.filler_corridor(foot, direction, depth)
    }

    /// `FillerCorridor.findPieceBox`: a corridor shortened until it clears the
    /// piece a full one would run into, one block longer than that.
    fn filler_corridor(
        &self,
        foot: IVec3,
        direction: Orientation,
        depth: i32,
    ) -> Option<StrongholdPiece> {
        let offset = IVec3::new(-1, -1, 0);
        let probe = orient_box_at(direction, foot, offset, 5, 5, 4);
        let collision = find_collision(&self.pieces, probe)?.bounds();
        if collision.min.y != probe.min.y {
            return None;
        }
        for length in (1..=2).rev() {
            if collision.intersects(orient_box_at(direction, foot, offset, 5, 5, length)) {
                continue;
            }
            let bounds = orient_box_at(direction, foot, offset, 5, 5, length + 1);
            if bounds.min.y <= 1 {
                return None;
            }
            let steps = match direction {
                Orientation::West | Orientation::East => bounds.max.x - bounds.min.x + 1,
                Orientation::North | Orientation::South => bounds.max.z - bounds.min.z + 1,
            };
            return Some(StrongholdPiece {
                kind: StrongholdKind::FillerCorridor { steps },
                entry_door: SmallDoor::Opening,
                bounds,
                orientation: direction,
                gen_depth: depth,
            });
        }
        None
    }

    /// Each type's `createPiece`: the box, its floor above 10, no collision,
    /// then the constructor's own draws; the library falls back to its short
    /// box before giving up.
    fn create(
        &self,
        rng: &mut LegacyRandom,
        kind: StrongholdKind,
        foot: IVec3,
        direction: Orientation,
        depth: i32,
    ) -> Option<StrongholdPiece> {
        use StrongholdKind::*;
        let (offset, size) = match kind {
            Straight { .. } | ChestCorridor => ([-1, -1], [5, 5, 7]),
            PrisonHall => ([-1, -1], [9, 5, 11]),
            LeftTurn | RightTurn => ([-1, -1], [5, 5, 5]),
            RoomCrossing { .. } => ([-4, -1], [11, 7, 11]),
            StraightStairsDown => ([-1, -7], [5, 11, 8]),
            StairsDown => ([-1, -7], [5, 11, 5]),
            FiveCrossing { .. } => ([-4, -3], [10, 9, 11]),
            Library { .. } => ([-4, -1], [14, 11, 15]),
            PortalRoom => ([-4, -1], [11, 8, 16]),
            Start | FillerCorridor { .. } => unreachable!("never drawn from the table"),
        };
        let offset = IVec3::new(offset[0], offset[1], 0);
        let ok = |bounds: BoundingBox| {
            bounds.min.y > LOWEST_Y && find_collision(&self.pieces, bounds).is_none()
        };
        let [width, height, depth_z] = size;
        let mut bounds = orient_box_at(direction, foot, offset, width, height, depth_z);
        if !ok(bounds) {
            if !matches!(kind, Library { .. }) {
                return None;
            }
            bounds = orient_box_at(direction, foot, offset, width, 6, depth_z);
            if !ok(bounds) {
                return None;
            }
        }
        let entry_door = match kind {
            PortalRoom => SmallDoor::Opening,
            _ => random_small_door(rng),
        };
        let kind = match kind {
            Straight { .. } => Straight {
                left: rng.next_i32_bound(2) == 0,
                right: rng.next_i32_bound(2) == 0,
            },
            RoomCrossing { .. } => RoomCrossing {
                variant: rng.next_i32_bound(5),
            },
            FiveCrossing { .. } => FiveCrossing {
                left_low: rng.next_bool(),
                left_high: rng.next_bool(),
                right_low: rng.next_bool(),
                right_high: rng.next_i32_bound(3) > 0,
            },
            Library { .. } => Library {
                tall: bounds.y_span() > 6,
            },
            other => other,
        };
        Some(StrongholdPiece {
            kind,
            entry_door,
            bounds,
            orientation: direction,
            gen_depth: depth,
        })
    }
}

/// `randomSmallDoor`.
fn random_small_door(rng: &mut LegacyRandom) -> SmallDoor {
    match rng.next_i32_bound(5) {
        2 => SmallDoor::WoodDoor,
        3 => SmallDoor::Grates,
        4 => SmallDoor::IronDoor,
        _ => SmallDoor::Opening,
    }
}
