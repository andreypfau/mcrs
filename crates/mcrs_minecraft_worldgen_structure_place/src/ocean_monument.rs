use mcrs_minecraft_core::Direction;
use mcrs_minecraft_random::Random;
use mcrs_minecraft_random::xoroshiro::XoroshiroRandom;
use mcrs_minecraft_worldgen_feature::compile::{BlockResolver, FeatureCompileError};
use mcrs_minecraft_worldgen_feature::placer::{StateMask, WorldGenVolume, WorldStates};
use mcrs_minecraft_worldgen_feature_place::entity::elder_guardian;
use mcrs_minecraft_worldgen_structure::piece::{
    MonumentRoom, MonumentRoomKind, OceanMonumentPiece,
};

use crate::canvas::{PieceCanvas, replaceable_by_structures};
use crate::{Oriented, block_mask, state};

#[derive(Clone, Debug)]
pub struct OceanMonumentBlocks {
    pub gray: Oriented,
    pub light: Oriented,
    pub black: Oriented,
    pub lamp: Oriented,
    pub gold: Oriented,
    pub wet_sponge: Oriented,
    pub air: Oriented,
    pub water: Oriented,
    /// Ice, packed ice, blue ice and water in every state: what a water box
    /// leaves standing.
    pub fill_keep: StateMask,
    pub replaceable_by_structures: StateMask,
}

impl OceanMonumentBlocks {
    pub fn compile(
        blocks: &dyn BlockResolver,
        world: &WorldStates,
    ) -> Result<Self, FeatureCompileError> {
        let oriented = |block: &str| Ok(Oriented::of(world, state(blocks, block, &[])?));
        Ok(OceanMonumentBlocks {
            gray: oriented("minecraft:prismarine")?,
            light: oriented("minecraft:prismarine_bricks")?,
            black: oriented("minecraft:dark_prismarine")?,
            lamp: oriented("minecraft:sea_lantern")?,
            gold: oriented("minecraft:gold_block")?,
            wet_sponge: oriented("minecraft:wet_sponge")?,
            air: oriented("minecraft:air")?,
            water: oriented("minecraft:water")?,
            fill_keep: block_mask(
                blocks,
                &[
                    "minecraft:ice",
                    "minecraft:packed_ice",
                    "minecraft:blue_ice",
                    "minecraft:water",
                ],
            )?,
            replaceable_by_structures: replaceable_by_structures(blocks, world)?,
        })
    }
}

const DOWN: usize = Direction::Down.id();
const UP: usize = Direction::Up.id();
const NORTH: usize = Direction::North.id();
const SOUTH: usize = Direction::South.id();
const WEST: usize = Direction::West.id();
const EAST: usize = Direction::East.id();

/// One child piece's brushes: the blocks, its canvas and the room graph.
struct Room<'a, 'b, W: WorldGenVolume> {
    b: &'a OceanMonumentBlocks,
    c: PieceCanvas<'b, W>,
    rooms: &'a [MonumentRoom],
}

impl<W: WorldGenVolume> Room<'_, '_, W> {
    fn solid(&mut self, state: &Oriented, min: [i32; 3], max: [i32; 3]) {
        self.c.generate_box(min, max, state, state, false);
    }

    fn place(&mut self, state: &Oriented, x: i32, y: i32, z: i32) {
        self.c.place(state, x, y, z);
    }

    fn room(&self, room: u8) -> &MonumentRoom {
        &self.rooms[usize::from(room)]
    }

    fn past(&self, room: u8, direction: usize) -> u8 {
        self.room(room).connections[direction].expect("an open face leads to a room")
    }

    fn open(&self, room: u8, direction: usize) -> bool {
        self.room(room).has_opening[direction]
    }

    fn has_up(&self, room: u8) -> bool {
        self.room(room).connections[UP].is_some()
    }

    fn above_ground_floor(&self, room: u8) -> bool {
        self.room(room).index / MonumentRoom::GRID_FLOOR > 0
    }

    /// `generateWaterBox`: air above the sea, water below, over anything but
    /// the kept blocks; every cell read before it is written.
    fn water_box(&mut self, min: [i32; 3], max: [i32; 3]) {
        let sea_level = self.c.volume.extent().sea_level;
        let [x0, y0, z0] = min;
        let [x1, y1, z1] = max;
        for y in y0..=y1 {
            for x in x0..=x1 {
                for z in z0..=z1 {
                    let block = self.c.get(x, y, z);
                    if self.b.fill_keep.contains(block.0 as usize) {
                        continue;
                    }
                    if self.c.world_pos(x, y, z).y >= sea_level {
                        self.c.place(&self.b.air, x, y, z);
                    } else {
                        self.c.place(&self.b.water, x, y, z);
                    }
                }
            }
        }
    }

    /// `generateBoxOnFillOnly`: the state where still water stands.
    fn box_on_fill_only(&mut self, min: [i32; 3], max: [i32; 3], target: &Oriented) {
        let [x0, y0, z0] = min;
        let [x1, y1, z1] = max;
        for y in y0..=y1 {
            for x in x0..=x1 {
                for z in z0..=z1 {
                    if self.c.get(x, y, z) == self.b.water.unoriented() {
                        self.c.place(target, x, y, z);
                    }
                }
            }
        }
    }

    fn default_floor(&mut self, xo: i32, zo: i32, down_opening: bool) {
        let (gray, light) = (self.b.gray, self.b.light);
        if down_opening {
            self.solid(&gray, [xo, 0, zo], [xo + 2, 0, zo + 7]);
            self.solid(&gray, [xo + 5, 0, zo], [xo + 7, 0, zo + 7]);
            self.solid(&gray, [xo + 3, 0, zo], [xo + 4, 0, zo + 2]);
            self.solid(&gray, [xo + 3, 0, zo + 5], [xo + 4, 0, zo + 7]);
            self.solid(&light, [xo + 3, 0, zo + 2], [xo + 4, 0, zo + 2]);
            self.solid(&light, [xo + 3, 0, zo + 5], [xo + 4, 0, zo + 5]);
            self.solid(&light, [xo + 2, 0, zo + 3], [xo + 2, 0, zo + 4]);
            self.solid(&light, [xo + 5, 0, zo + 3], [xo + 5, 0, zo + 4]);
        } else {
            self.solid(&gray, [xo, 0, zo], [xo + 7, 0, zo + 7]);
        }
    }

    /// `chunkIntersects`: whether the piece-local x/z rectangle meets the clip.
    fn chunk_intersects(&self, x0: i32, z0: i32, x1: i32, z1: i32) -> bool {
        let a = self.c.world_pos(x0, 0, z0);
        let b = self.c.world_pos(x1, 0, z1);
        let clip = self.c.clip;
        clip.max.x >= a.x.min(b.x)
            && clip.min.x <= a.x.max(b.x)
            && clip.max.z >= a.z.min(b.z)
            && clip.min.z <= a.z.max(b.z)
    }

    fn spawn_elder(
        &mut self,
        rng: &mut XoroshiroRandom,
        x: i32,
        y: i32,
        z: i32,
    ) {
        let pos = self.c.world_pos(x, y, z);
        if self.c.clip.is_inside(pos) {
            self.c.spawns.push(elder_guardian(pos, rng));
        }
    }
}

/// `MonumentBuilding.postProcess` for one column, its children included.
pub fn paint_ocean_monument<W: WorldGenVolume>(
    b: &OceanMonumentBlocks,
    piece: &OceanMonumentPiece,
    c: &mut PieceCanvas<'_, W>,
    rng: &mut XoroshiroRandom,
) {
    let clip = c.clip;
    let orientation = c.orientation;
    let mut r = Room {
        b,
        c: PieceCanvas {
            volume: &mut *c.volume,
            entities: &mut *c.entities,
            spawns: &mut *c.spawns,
            bounds: piece.bounds,
            orientation,
            clip,
        },
        rooms: &piece.rooms,
    };
    building(&mut r);
    for child in &piece.children {
        if !child.bounds.intersects(clip) {
            continue;
        }
        let mut r = Room {
            b,
            c: PieceCanvas {
                volume: &mut *c.volume,
                entities: &mut *c.entities,
                spawns: &mut *c.spawns,
                bounds: child.bounds,
                orientation,
                clip,
            },
            rooms: &piece.rooms,
        };
        match child.kind {
            MonumentRoomKind::Entry { room } => entry_room(&mut r, room),
            MonumentRoomKind::Core { .. } => core_room(&mut r),
            MonumentRoomKind::DoubleX { room } => double_x_room(&mut r, room),
            MonumentRoomKind::DoubleXY { room } => double_xy_room(&mut r, room),
            MonumentRoomKind::DoubleY { room } => double_y_room(&mut r, room),
            MonumentRoomKind::DoubleYZ { room } => double_yz_room(&mut r, room),
            MonumentRoomKind::DoubleZ { room } => double_z_room(&mut r, room),
            MonumentRoomKind::Simple { room, main_design } => {
                simple_room(&mut r, rng, room, main_design)
            }
            MonumentRoomKind::SimpleTop { room } => simple_top_room(&mut r, rng, room),
            MonumentRoomKind::Wing { main_design } => wing_room(&mut r, rng, main_design),
            MonumentRoomKind::Penthouse => penthouse(&mut r, rng),
        }
    }
}

fn building<W: WorldGenVolume>(r: &mut Room<'_, '_, W>) {
    let (gray, light, black, lamp) = (r.b.gray, r.b.light, r.b.black, r.b.lamp);
    let water_height = r.c.volume.extent().sea_level.max(64) - r.c.bounds.min.y;
    r.water_box([0, 0, 0], [58, water_height, 58]);
    wing(r, false, 0);
    wing(r, true, 33);
    if r.chunk_intersects(22, 5, 35, 17) {
        r.water_box([25, 0, 0], [32, 8, 20]);
        for i in 0..4 {
            let z = 5 + i * 4;
            r.solid(&light, [24, 2, z], [24, 4, z]);
            r.solid(&light, [22, 4, z], [23, 4, z]);
            r.place(&light, 25, 5, z);
            r.place(&light, 26, 6, z);
            r.place(&lamp, 26, 5, z);
            r.solid(&light, [33, 2, z], [33, 4, z]);
            r.solid(&light, [34, 4, z], [35, 4, z]);
            r.place(&light, 32, 5, z);
            r.place(&light, 31, 6, z);
            r.place(&lamp, 31, 5, z);
            r.solid(&gray, [27, 6, z], [30, 6, z]);
        }
    }
    if r.chunk_intersects(15, 20, 42, 21) {
        r.solid(&gray, [15, 0, 21], [42, 0, 21]);
        r.water_box([26, 1, 21], [31, 3, 21]);
        r.solid(&gray, [21, 12, 21], [36, 12, 21]);
        r.solid(&gray, [17, 11, 21], [40, 11, 21]);
        r.solid(&gray, [16, 10, 21], [41, 10, 21]);
        r.solid(&gray, [15, 7, 21], [42, 9, 21]);
        r.solid(&gray, [16, 6, 21], [41, 6, 21]);
        r.solid(&gray, [17, 5, 21], [40, 5, 21]);
        r.solid(&gray, [21, 4, 21], [36, 4, 21]);
        r.solid(&gray, [22, 3, 21], [26, 3, 21]);
        r.solid(&gray, [31, 3, 21], [35, 3, 21]);
        r.solid(&gray, [23, 2, 21], [25, 2, 21]);
        r.solid(&gray, [32, 2, 21], [34, 2, 21]);
        r.solid(&light, [28, 4, 20], [29, 4, 21]);
        r.place(&light, 27, 3, 21);
        r.place(&light, 30, 3, 21);
        r.place(&light, 26, 2, 21);
        r.place(&light, 31, 2, 21);
        r.place(&light, 25, 1, 21);
        r.place(&light, 32, 1, 21);
        for i in 0..7 {
            r.place(&black, 28 - i, 6 + i, 21);
            r.place(&black, 29 + i, 6 + i, 21);
        }
        for i in 0..4 {
            r.place(&black, 28 - i, 9 + i, 21);
            r.place(&black, 29 + i, 9 + i, 21);
        }
        r.place(&black, 28, 12, 21);
        r.place(&black, 29, 12, 21);
        for i in 0..3 {
            r.place(&black, 22 - i * 2, 8, 21);
            r.place(&black, 22 - i * 2, 9, 21);
            r.place(&black, 35 + i * 2, 8, 21);
            r.place(&black, 35 + i * 2, 9, 21);
        }
        r.water_box([15, 13, 21], [42, 15, 21]);
        r.water_box([15, 1, 21], [15, 6, 21]);
        r.water_box([16, 1, 21], [16, 5, 21]);
        r.water_box([17, 1, 21], [20, 4, 21]);
        r.water_box([21, 1, 21], [21, 3, 21]);
        r.water_box([22, 1, 21], [22, 2, 21]);
        r.water_box([23, 1, 21], [24, 1, 21]);
        r.water_box([42, 1, 21], [42, 6, 21]);
        r.water_box([41, 1, 21], [41, 5, 21]);
        r.water_box([37, 1, 21], [40, 4, 21]);
        r.water_box([36, 1, 21], [36, 3, 21]);
        r.water_box([33, 1, 21], [34, 1, 21]);
        r.water_box([35, 1, 21], [35, 2, 21]);
    }
    if r.chunk_intersects(21, 21, 36, 36) {
        r.solid(&gray, [21, 0, 22], [36, 0, 36]);
        r.water_box([21, 1, 22], [36, 23, 36]);
        for i in 0..4 {
            r.solid(&light, [21 + i, 13 + i, 21 + i], [36 - i, 13 + i, 21 + i]);
            r.solid(&light, [21 + i, 13 + i, 36 - i], [36 - i, 13 + i, 36 - i]);
            r.solid(&light, [21 + i, 13 + i, 22 + i], [21 + i, 13 + i, 35 - i]);
            r.solid(&light, [36 - i, 13 + i, 22 + i], [36 - i, 13 + i, 35 - i]);
        }
        r.solid(&gray, [25, 16, 25], [32, 16, 32]);
        r.solid(&light, [25, 17, 25], [25, 19, 25]);
        r.solid(&light, [32, 17, 25], [32, 19, 25]);
        r.solid(&light, [25, 17, 32], [25, 19, 32]);
        r.solid(&light, [32, 17, 32], [32, 19, 32]);
        r.place(&light, 26, 20, 26);
        r.place(&light, 27, 21, 27);
        r.place(&lamp, 27, 20, 27);
        r.place(&light, 26, 20, 31);
        r.place(&light, 27, 21, 30);
        r.place(&lamp, 27, 20, 30);
        r.place(&light, 31, 20, 31);
        r.place(&light, 30, 21, 30);
        r.place(&lamp, 30, 20, 30);
        r.place(&light, 31, 20, 26);
        r.place(&light, 30, 21, 27);
        r.place(&lamp, 30, 20, 27);
        r.solid(&gray, [28, 21, 27], [29, 21, 27]);
        r.solid(&gray, [27, 21, 28], [27, 21, 29]);
        r.solid(&gray, [28, 21, 30], [29, 21, 30]);
        r.solid(&gray, [30, 21, 28], [30, 21, 29]);
    }
    lower_wall(r);
    middle_wall(r);
    upper_wall(r);
    for pillar_x in 0..7 {
        let mut pillar_z = 0;
        while pillar_z < 7 {
            if pillar_z == 0 && pillar_x == 3 {
                pillar_z = 6;
            }
            let bx = pillar_x * 9;
            let bz = pillar_z * 9;
            for w in 0..4 {
                for d in 0..4 {
                    r.place(&light, bx + w, 0, bz + d);
                    r.c.fill_column_down(
                        &r.b.replaceable_by_structures,
                        light.unoriented(),
                        bx + w,
                        -1,
                        bz + d,
                    );
                }
            }
            if pillar_x != 0 && pillar_x != 6 {
                pillar_z += 6;
            } else {
                pillar_z += 1;
            }
        }
    }
    for i in 0..5 {
        r.water_box([-1 - i, i * 2, -1 - i], [-1 - i, 23, 58 + i]);
        r.water_box([58 + i, i * 2, -1 - i], [58 + i, 23, 58 + i]);
        r.water_box([-i, i * 2, -1 - i], [57 + i, 23, -1 - i]);
        r.water_box([-i, i * 2, 58 + i], [57 + i, 23, 58 + i]);
    }
}

fn wing<W: WorldGenVolume>(r: &mut Room<'_, '_, W>, flipped: bool, xoff: i32) {
    if !r.chunk_intersects(xoff, 0, xoff + 23, 20) {
        return;
    }
    let (gray, light) = (r.b.gray, r.b.light);
    r.solid(&gray, [xoff, 0, 0], [xoff + 24, 0, 20]);
    r.water_box([xoff, 1, 0], [xoff + 24, 10, 20]);
    for i in 0..4 {
        r.solid(&light, [xoff + i, i + 1, i], [xoff + i, i + 1, 20]);
        r.solid(
            &light,
            [xoff + i + 7, i + 5, i + 7],
            [xoff + i + 7, i + 5, 20],
        );
        r.solid(
            &light,
            [xoff + 17 - i, i + 5, i + 7],
            [xoff + 17 - i, i + 5, 20],
        );
        r.solid(
            &light,
            [xoff + 24 - i, i + 1, i],
            [xoff + 24 - i, i + 1, 20],
        );
        r.solid(&light, [xoff + i + 1, i + 1, i], [xoff + 23 - i, i + 1, i]);
        r.solid(
            &light,
            [xoff + i + 8, i + 5, i + 7],
            [xoff + 16 - i, i + 5, i + 7],
        );
    }
    r.solid(&gray, [xoff + 4, 4, 4], [xoff + 6, 4, 20]);
    r.solid(&gray, [xoff + 7, 4, 4], [xoff + 17, 4, 6]);
    r.solid(&gray, [xoff + 18, 4, 4], [xoff + 20, 4, 20]);
    r.solid(&gray, [xoff + 11, 8, 11], [xoff + 13, 8, 20]);
    r.place(&light, xoff + 12, 9, 12);
    r.place(&light, xoff + 12, 9, 15);
    r.place(&light, xoff + 12, 9, 18);
    let left = xoff + if flipped { 19 } else { 5 };
    let right = xoff + if flipped { 5 } else { 19 };
    for z in (5..=20).rev().step_by(3) {
        r.place(&light, left, 5, z);
    }
    for z in (7..=19).rev().step_by(3) {
        r.place(&light, right, 5, z);
    }
    for i in 0..4 {
        let x = if flipped {
            xoff + 24 - (17 - i * 3)
        } else {
            xoff + 17 - i * 3
        };
        r.place(&light, x, 5, 5);
    }
    r.place(&light, right, 5, 5);
    r.solid(&gray, [xoff + 11, 1, 12], [xoff + 13, 7, 12]);
    r.solid(&gray, [xoff + 12, 1, 11], [xoff + 12, 7, 13]);
}

fn lower_wall<W: WorldGenVolume>(r: &mut Room<'_, '_, W>) {
    let (gray, light) = (r.b.gray, r.b.light);
    if r.chunk_intersects(0, 21, 6, 58) {
        r.solid(&gray, [0, 0, 21], [6, 0, 57]);
        r.water_box([0, 1, 21], [6, 7, 57]);
        r.solid(&gray, [4, 4, 21], [6, 4, 53]);
        for i in 0..4 {
            r.solid(&light, [i, i + 1, 21], [i, i + 1, 57 - i]);
        }
        for z in (23..53).step_by(3) {
            r.place(&light, 5, 5, z);
        }
        r.place(&light, 5, 5, 52);
        for i in 0..4 {
            r.solid(&light, [i, i + 1, 21], [i, i + 1, 57 - i]);
        }
        r.solid(&gray, [4, 1, 52], [6, 3, 52]);
        r.solid(&gray, [5, 1, 51], [5, 3, 53]);
    }
    if r.chunk_intersects(51, 21, 58, 58) {
        r.solid(&gray, [51, 0, 21], [57, 0, 57]);
        r.water_box([51, 1, 21], [57, 7, 57]);
        r.solid(&gray, [51, 4, 21], [53, 4, 53]);
        for i in 0..4 {
            r.solid(&light, [57 - i, i + 1, 21], [57 - i, i + 1, 57 - i]);
        }
        for z in (23..53).step_by(3) {
            r.place(&light, 52, 5, z);
        }
        r.place(&light, 52, 5, 52);
        r.solid(&gray, [51, 1, 52], [53, 3, 52]);
        r.solid(&gray, [52, 1, 51], [52, 3, 53]);
    }
    if r.chunk_intersects(0, 51, 57, 57) {
        r.solid(&gray, [7, 0, 51], [50, 0, 57]);
        r.water_box([7, 1, 51], [50, 10, 57]);
        for i in 0..4 {
            r.solid(&light, [i + 1, i + 1, 57 - i], [56 - i, i + 1, 57 - i]);
        }
    }
}

fn middle_wall<W: WorldGenVolume>(r: &mut Room<'_, '_, W>) {
    let (gray, light) = (r.b.gray, r.b.light);
    if r.chunk_intersects(7, 21, 13, 50) {
        r.solid(&gray, [7, 0, 21], [13, 0, 50]);
        r.water_box([7, 1, 21], [13, 10, 50]);
        r.solid(&gray, [11, 8, 21], [13, 8, 53]);
        for i in 0..4 {
            r.solid(&light, [i + 7, i + 5, 21], [i + 7, i + 5, 54]);
        }
        for z in (21..=45).step_by(3) {
            r.place(&light, 12, 9, z);
        }
    }
    if r.chunk_intersects(44, 21, 50, 54) {
        r.solid(&gray, [44, 0, 21], [50, 0, 50]);
        r.water_box([44, 1, 21], [50, 10, 50]);
        r.solid(&gray, [44, 8, 21], [46, 8, 53]);
        for i in 0..4 {
            r.solid(&light, [50 - i, i + 5, 21], [50 - i, i + 5, 54]);
        }
        for z in (21..=45).step_by(3) {
            r.place(&light, 45, 9, z);
        }
    }
    if r.chunk_intersects(8, 44, 49, 54) {
        r.solid(&gray, [14, 0, 44], [43, 0, 50]);
        r.water_box([14, 1, 44], [43, 10, 50]);
        for x in (12..=45).step_by(3) {
            r.place(&light, x, 9, 45);
            r.place(&light, x, 9, 52);
            if matches!(x, 12 | 18 | 24 | 33 | 39 | 45) {
                r.place(&light, x, 9, 47);
                r.place(&light, x, 9, 50);
                r.place(&light, x, 10, 45);
                r.place(&light, x, 10, 46);
                r.place(&light, x, 10, 51);
                r.place(&light, x, 10, 52);
                r.place(&light, x, 11, 47);
                r.place(&light, x, 11, 50);
                r.place(&light, x, 12, 48);
                r.place(&light, x, 12, 49);
            }
        }
        for i in 0..3 {
            r.solid(&gray, [8 + i, 5 + i, 54], [49 - i, 5 + i, 54]);
        }
        r.solid(&light, [11, 8, 54], [46, 8, 54]);
        r.solid(&gray, [14, 8, 44], [43, 8, 53]);
    }
}

fn upper_wall<W: WorldGenVolume>(r: &mut Room<'_, '_, W>) {
    let (gray, light) = (r.b.gray, r.b.light);
    if r.chunk_intersects(14, 21, 20, 43) {
        r.solid(&gray, [14, 0, 21], [20, 0, 43]);
        r.water_box([14, 1, 22], [20, 14, 43]);
        r.solid(&gray, [18, 12, 22], [20, 12, 39]);
        r.solid(&light, [18, 12, 21], [20, 12, 21]);
        for i in 0..4 {
            r.solid(&light, [i + 14, i + 9, 21], [i + 14, i + 9, 43 - i]);
        }
        for z in (23..=39).step_by(3) {
            r.place(&light, 19, 13, z);
        }
    }
    if r.chunk_intersects(37, 21, 43, 43) {
        r.solid(&gray, [37, 0, 21], [43, 0, 43]);
        r.water_box([37, 1, 22], [43, 14, 43]);
        r.solid(&gray, [37, 12, 22], [39, 12, 39]);
        r.solid(&light, [37, 12, 21], [39, 12, 21]);
        for i in 0..4 {
            r.solid(&light, [43 - i, i + 9, 21], [43 - i, i + 9, 43 - i]);
        }
        for z in (23..=39).step_by(3) {
            r.place(&light, 38, 13, z);
        }
    }
    if r.chunk_intersects(15, 37, 42, 43) {
        r.solid(&gray, [21, 0, 37], [36, 0, 43]);
        r.water_box([21, 1, 37], [36, 14, 43]);
        r.solid(&gray, [21, 12, 37], [36, 12, 39]);
        for i in 0..4 {
            r.solid(&light, [15 + i, i + 9, 43 - i], [42 - i, i + 9, 43 - i]);
        }
        for x in (21..=36).step_by(3) {
            r.place(&light, x, 13, 38);
        }
    }
}

fn core_room<W: WorldGenVolume>(r: &mut Room<'_, '_, W>) {
    let (gray, light, black, lamp, gold) = (r.b.gray, r.b.light, r.b.black, r.b.lamp, r.b.gold);
    r.box_on_fill_only([1, 8, 0], [14, 8, 14], &gray);
    r.solid(&light, [0, 7, 0], [0, 7, 15]);
    r.solid(&light, [15, 7, 0], [15, 7, 15]);
    r.solid(&light, [1, 7, 0], [15, 7, 0]);
    r.solid(&light, [1, 7, 15], [14, 7, 15]);
    for y in 1..=6 {
        let block = if y == 2 || y == 6 { gray } else { light };
        for x in (0..=15).step_by(15) {
            r.solid(&block, [x, y, 0], [x, y, 1]);
            r.solid(&block, [x, y, 6], [x, y, 9]);
            r.solid(&block, [x, y, 14], [x, y, 15]);
        }
        r.solid(&block, [1, y, 0], [1, y, 0]);
        r.solid(&block, [6, y, 0], [9, y, 0]);
        r.solid(&block, [14, y, 0], [14, y, 0]);
        r.solid(&block, [1, y, 15], [14, y, 15]);
    }
    r.solid(&black, [6, 3, 6], [9, 6, 9]);
    r.solid(&gold, [7, 4, 7], [8, 5, 8]);
    for y in (3..=6).step_by(3) {
        for x in (6..=9).step_by(3) {
            r.place(&lamp, x, y, 6);
            r.place(&lamp, x, y, 9);
        }
    }
    for (min, max) in [
        ([5, 1, 6], [5, 2, 6]),
        ([5, 1, 9], [5, 2, 9]),
        ([10, 1, 6], [10, 2, 6]),
        ([10, 1, 9], [10, 2, 9]),
        ([6, 1, 5], [6, 2, 5]),
        ([9, 1, 5], [9, 2, 5]),
        ([6, 1, 10], [6, 2, 10]),
        ([9, 1, 10], [9, 2, 10]),
        ([5, 2, 5], [5, 6, 5]),
        ([5, 2, 10], [5, 6, 10]),
        ([10, 2, 5], [10, 6, 5]),
        ([10, 2, 10], [10, 6, 10]),
        ([5, 7, 1], [5, 7, 6]),
        ([10, 7, 1], [10, 7, 6]),
        ([5, 7, 9], [5, 7, 14]),
        ([10, 7, 9], [10, 7, 14]),
        ([1, 7, 5], [6, 7, 5]),
        ([1, 7, 10], [6, 7, 10]),
        ([9, 7, 5], [14, 7, 5]),
        ([9, 7, 10], [14, 7, 10]),
        ([2, 1, 2], [2, 1, 3]),
        ([3, 1, 2], [3, 1, 2]),
        ([13, 1, 2], [13, 1, 3]),
        ([12, 1, 2], [12, 1, 2]),
        ([2, 1, 12], [2, 1, 13]),
        ([3, 1, 13], [3, 1, 13]),
        ([13, 1, 12], [13, 1, 13]),
        ([12, 1, 13], [12, 1, 13]),
    ] {
        r.solid(&light, min, max);
    }
}

fn double_x_room<W: WorldGenVolume>(r: &mut Room<'_, '_, W>, west: u8) {
    let (gray, light, lamp) = (r.b.gray, r.b.light, r.b.lamp);
    let east = r.past(west, EAST);
    if r.above_ground_floor(west) {
        r.default_floor(8, 0, r.open(east, DOWN));
        r.default_floor(0, 0, r.open(west, DOWN));
    }
    if !r.has_up(west) {
        r.box_on_fill_only([1, 4, 1], [7, 4, 6], &gray);
    }
    if !r.has_up(east) {
        r.box_on_fill_only([8, 4, 1], [14, 4, 6], &gray);
    }
    r.solid(&light, [0, 3, 0], [0, 3, 7]);
    r.solid(&light, [15, 3, 0], [15, 3, 7]);
    r.solid(&light, [1, 3, 0], [15, 3, 0]);
    r.solid(&light, [1, 3, 7], [14, 3, 7]);
    r.solid(&gray, [0, 2, 0], [0, 2, 7]);
    r.solid(&gray, [15, 2, 0], [15, 2, 7]);
    r.solid(&gray, [1, 2, 0], [15, 2, 0]);
    r.solid(&gray, [1, 2, 7], [14, 2, 7]);
    r.solid(&light, [0, 1, 0], [0, 1, 7]);
    r.solid(&light, [15, 1, 0], [15, 1, 7]);
    r.solid(&light, [1, 1, 0], [15, 1, 0]);
    r.solid(&light, [1, 1, 7], [14, 1, 7]);
    r.solid(&light, [5, 1, 0], [10, 1, 4]);
    r.solid(&gray, [6, 2, 0], [9, 2, 3]);
    r.solid(&light, [5, 3, 0], [10, 3, 4]);
    r.place(&lamp, 6, 2, 3);
    r.place(&lamp, 9, 2, 3);
    if r.open(west, SOUTH) {
        r.water_box([3, 1, 0], [4, 2, 0]);
    }
    if r.open(west, NORTH) {
        r.water_box([3, 1, 7], [4, 2, 7]);
    }
    if r.open(west, WEST) {
        r.water_box([0, 1, 3], [0, 2, 4]);
    }
    if r.open(east, SOUTH) {
        r.water_box([11, 1, 0], [12, 2, 0]);
    }
    if r.open(east, NORTH) {
        r.water_box([11, 1, 7], [12, 2, 7]);
    }
    if r.open(east, EAST) {
        r.water_box([15, 1, 3], [15, 2, 4]);
    }
}

fn double_xy_room<W: WorldGenVolume>(r: &mut Room<'_, '_, W>, west: u8) {
    let (gray, light, lamp) = (r.b.gray, r.b.light, r.b.lamp);
    let east = r.past(west, EAST);
    let west_up = r.past(west, UP);
    let east_up = r.past(east, UP);
    if r.above_ground_floor(west) {
        r.default_floor(8, 0, r.open(east, DOWN));
        r.default_floor(0, 0, r.open(west, DOWN));
    }
    if !r.has_up(west_up) {
        r.box_on_fill_only([1, 8, 1], [7, 8, 6], &gray);
    }
    if !r.has_up(east_up) {
        r.box_on_fill_only([8, 8, 1], [14, 8, 6], &gray);
    }
    for y in 1..=7 {
        let block = if y == 2 || y == 6 { gray } else { light };
        r.solid(&block, [0, y, 0], [0, y, 7]);
        r.solid(&block, [15, y, 0], [15, y, 7]);
        r.solid(&block, [1, y, 0], [15, y, 0]);
        r.solid(&block, [1, y, 7], [14, y, 7]);
    }
    for (min, max) in [
        ([2, 1, 3], [2, 7, 4]),
        ([3, 1, 2], [4, 7, 2]),
        ([3, 1, 5], [4, 7, 5]),
        ([13, 1, 3], [13, 7, 4]),
        ([11, 1, 2], [12, 7, 2]),
        ([11, 1, 5], [12, 7, 5]),
        ([5, 1, 3], [5, 3, 4]),
        ([10, 1, 3], [10, 3, 4]),
        ([5, 7, 2], [10, 7, 5]),
        ([5, 5, 2], [5, 7, 2]),
        ([10, 5, 2], [10, 7, 2]),
        ([5, 5, 5], [5, 7, 5]),
        ([10, 5, 5], [10, 7, 5]),
    ] {
        r.solid(&light, min, max);
    }
    r.place(&light, 6, 6, 2);
    r.place(&light, 9, 6, 2);
    r.place(&light, 6, 6, 5);
    r.place(&light, 9, 6, 5);
    r.solid(&light, [5, 4, 3], [6, 4, 4]);
    r.solid(&light, [9, 4, 3], [10, 4, 4]);
    r.place(&lamp, 5, 4, 2);
    r.place(&lamp, 5, 4, 5);
    r.place(&lamp, 10, 4, 2);
    r.place(&lamp, 10, 4, 5);
    if r.open(west, SOUTH) {
        r.water_box([3, 1, 0], [4, 2, 0]);
    }
    if r.open(west, NORTH) {
        r.water_box([3, 1, 7], [4, 2, 7]);
    }
    if r.open(west, WEST) {
        r.water_box([0, 1, 3], [0, 2, 4]);
    }
    if r.open(east, SOUTH) {
        r.water_box([11, 1, 0], [12, 2, 0]);
    }
    if r.open(east, NORTH) {
        r.water_box([11, 1, 7], [12, 2, 7]);
    }
    if r.open(east, EAST) {
        r.water_box([15, 1, 3], [15, 2, 4]);
    }
    if r.open(west_up, SOUTH) {
        r.water_box([3, 5, 0], [4, 6, 0]);
    }
    if r.open(west_up, NORTH) {
        r.water_box([3, 5, 7], [4, 6, 7]);
    }
    if r.open(west_up, WEST) {
        r.water_box([0, 5, 3], [0, 6, 4]);
    }
    if r.open(east_up, SOUTH) {
        r.water_box([11, 5, 0], [12, 6, 0]);
    }
    if r.open(east_up, NORTH) {
        r.water_box([11, 5, 7], [12, 6, 7]);
    }
    if r.open(east_up, EAST) {
        r.water_box([15, 5, 3], [15, 6, 4]);
    }
}

fn double_y_room<W: WorldGenVolume>(r: &mut Room<'_, '_, W>, room: u8) {
    let (gray, light) = (r.b.gray, r.b.light);
    if r.above_ground_floor(room) {
        r.default_floor(0, 0, r.open(room, DOWN));
    }
    let above = r.past(room, UP);
    if !r.has_up(above) {
        r.box_on_fill_only([1, 8, 1], [6, 8, 6], &gray);
    }
    for (min, max) in [
        ([0, 4, 0], [0, 4, 7]),
        ([7, 4, 0], [7, 4, 7]),
        ([1, 4, 0], [6, 4, 0]),
        ([1, 4, 7], [6, 4, 7]),
        ([2, 4, 1], [2, 4, 2]),
        ([1, 4, 2], [1, 4, 2]),
        ([5, 4, 1], [5, 4, 2]),
        ([6, 4, 2], [6, 4, 2]),
        ([2, 4, 5], [2, 4, 6]),
        ([1, 4, 5], [1, 4, 5]),
        ([5, 4, 5], [5, 4, 6]),
        ([6, 4, 5], [6, 4, 5]),
    ] {
        r.solid(&light, min, max);
    }
    let mut definition = room;
    for y in (1..=5).step_by(4) {
        if r.open(definition, SOUTH) {
            r.solid(&light, [2, y, 0], [2, y + 2, 0]);
            r.solid(&light, [5, y, 0], [5, y + 2, 0]);
            r.solid(&light, [3, y + 2, 0], [4, y + 2, 0]);
        } else {
            r.solid(&light, [0, y, 0], [7, y + 2, 0]);
            r.solid(&gray, [0, y + 1, 0], [7, y + 1, 0]);
        }
        if r.open(definition, NORTH) {
            r.solid(&light, [2, y, 7], [2, y + 2, 7]);
            r.solid(&light, [5, y, 7], [5, y + 2, 7]);
            r.solid(&light, [3, y + 2, 7], [4, y + 2, 7]);
        } else {
            r.solid(&light, [0, y, 7], [7, y + 2, 7]);
            r.solid(&gray, [0, y + 1, 7], [7, y + 1, 7]);
        }
        if r.open(definition, WEST) {
            r.solid(&light, [0, y, 2], [0, y + 2, 2]);
            r.solid(&light, [0, y, 5], [0, y + 2, 5]);
            r.solid(&light, [0, y + 2, 3], [0, y + 2, 4]);
        } else {
            r.solid(&light, [0, y, 0], [0, y + 2, 7]);
            r.solid(&gray, [0, y + 1, 0], [0, y + 1, 7]);
        }
        if r.open(definition, EAST) {
            r.solid(&light, [7, y, 2], [7, y + 2, 2]);
            r.solid(&light, [7, y, 5], [7, y + 2, 5]);
            r.solid(&light, [7, y + 2, 3], [7, y + 2, 4]);
        } else {
            r.solid(&light, [7, y, 0], [7, y + 2, 7]);
            r.solid(&gray, [7, y + 1, 0], [7, y + 1, 7]);
        }
        definition = above;
    }
}

fn double_yz_room<W: WorldGenVolume>(r: &mut Room<'_, '_, W>, south: u8) {
    let (gray, light, black, lamp) = (r.b.gray, r.b.light, r.b.black, r.b.lamp);
    let north = r.past(south, NORTH);
    let north_up = r.past(north, UP);
    let south_up = r.past(south, UP);
    if r.above_ground_floor(south) {
        r.default_floor(0, 8, r.open(north, DOWN));
        r.default_floor(0, 0, r.open(south, DOWN));
    }
    if !r.has_up(south_up) {
        r.box_on_fill_only([1, 8, 1], [6, 8, 7], &gray);
    }
    if !r.has_up(north_up) {
        r.box_on_fill_only([1, 8, 8], [6, 8, 14], &gray);
    }
    for y in 1..=7 {
        let block = if y == 2 || y == 6 { gray } else { light };
        r.solid(&block, [0, y, 0], [0, y, 15]);
        r.solid(&block, [7, y, 0], [7, y, 15]);
        r.solid(&block, [1, y, 0], [6, y, 0]);
        r.solid(&block, [1, y, 15], [6, y, 15]);
    }
    for y in 1..=7 {
        let block = if y == 2 || y == 6 { lamp } else { black };
        r.solid(&block, [3, y, 7], [4, y, 8]);
    }
    if r.open(south, SOUTH) {
        r.water_box([3, 1, 0], [4, 2, 0]);
    }
    if r.open(south, EAST) {
        r.water_box([7, 1, 3], [7, 2, 4]);
    }
    if r.open(south, WEST) {
        r.water_box([0, 1, 3], [0, 2, 4]);
    }
    if r.open(north, NORTH) {
        r.water_box([3, 1, 15], [4, 2, 15]);
    }
    if r.open(north, WEST) {
        r.water_box([0, 1, 11], [0, 2, 12]);
    }
    if r.open(north, EAST) {
        r.water_box([7, 1, 11], [7, 2, 12]);
    }
    if r.open(south_up, SOUTH) {
        r.water_box([3, 5, 0], [4, 6, 0]);
    }
    if r.open(south_up, EAST) {
        r.water_box([7, 5, 3], [7, 6, 4]);
        r.solid(&light, [5, 4, 2], [6, 4, 5]);
        r.solid(&light, [6, 1, 2], [6, 3, 2]);
        r.solid(&light, [6, 1, 5], [6, 3, 5]);
    }
    if r.open(south_up, WEST) {
        r.water_box([0, 5, 3], [0, 6, 4]);
        r.solid(&light, [1, 4, 2], [2, 4, 5]);
        r.solid(&light, [1, 1, 2], [1, 3, 2]);
        r.solid(&light, [1, 1, 5], [1, 3, 5]);
    }
    if r.open(north_up, NORTH) {
        r.water_box([3, 5, 15], [4, 6, 15]);
    }
    if r.open(north_up, WEST) {
        r.water_box([0, 5, 11], [0, 6, 12]);
        r.solid(&light, [1, 4, 10], [2, 4, 13]);
        r.solid(&light, [1, 1, 10], [1, 3, 10]);
        r.solid(&light, [1, 1, 13], [1, 3, 13]);
    }
    if r.open(north_up, EAST) {
        r.water_box([7, 5, 11], [7, 6, 12]);
        r.solid(&light, [5, 4, 10], [6, 4, 13]);
        r.solid(&light, [6, 1, 10], [6, 3, 10]);
        r.solid(&light, [6, 1, 13], [6, 3, 13]);
    }
}

fn double_z_room<W: WorldGenVolume>(r: &mut Room<'_, '_, W>, south: u8) {
    let (gray, light, lamp) = (r.b.gray, r.b.light, r.b.lamp);
    let north = r.past(south, NORTH);
    if r.above_ground_floor(south) {
        r.default_floor(0, 8, r.open(north, DOWN));
        r.default_floor(0, 0, r.open(south, DOWN));
    }
    if !r.has_up(south) {
        r.box_on_fill_only([1, 4, 1], [6, 4, 7], &gray);
    }
    if !r.has_up(north) {
        r.box_on_fill_only([1, 4, 8], [6, 4, 14], &gray);
    }
    r.solid(&light, [0, 3, 0], [0, 3, 15]);
    r.solid(&light, [7, 3, 0], [7, 3, 15]);
    r.solid(&light, [1, 3, 0], [7, 3, 0]);
    r.solid(&light, [1, 3, 15], [6, 3, 15]);
    r.solid(&gray, [0, 2, 0], [0, 2, 15]);
    r.solid(&gray, [7, 2, 0], [7, 2, 15]);
    r.solid(&gray, [1, 2, 0], [7, 2, 0]);
    r.solid(&gray, [1, 2, 15], [6, 2, 15]);
    for (min, max) in [
        ([0, 1, 0], [0, 1, 15]),
        ([7, 1, 0], [7, 1, 15]),
        ([1, 1, 0], [7, 1, 0]),
        ([1, 1, 15], [6, 1, 15]),
        ([1, 1, 1], [1, 1, 2]),
        ([6, 1, 1], [6, 1, 2]),
        ([1, 3, 1], [1, 3, 2]),
        ([6, 3, 1], [6, 3, 2]),
        ([1, 1, 13], [1, 1, 14]),
        ([6, 1, 13], [6, 1, 14]),
        ([1, 3, 13], [1, 3, 14]),
        ([6, 3, 13], [6, 3, 14]),
        ([2, 1, 6], [2, 3, 6]),
        ([5, 1, 6], [5, 3, 6]),
        ([2, 1, 9], [2, 3, 9]),
        ([5, 1, 9], [5, 3, 9]),
        ([3, 2, 6], [4, 2, 6]),
        ([3, 2, 9], [4, 2, 9]),
        ([2, 2, 7], [2, 2, 8]),
        ([5, 2, 7], [5, 2, 8]),
    ] {
        r.solid(&light, min, max);
    }
    r.place(&lamp, 2, 2, 5);
    r.place(&lamp, 5, 2, 5);
    r.place(&lamp, 2, 2, 10);
    r.place(&lamp, 5, 2, 10);
    r.place(&light, 2, 3, 5);
    r.place(&light, 5, 3, 5);
    r.place(&light, 2, 3, 10);
    r.place(&light, 5, 3, 10);
    if r.open(south, SOUTH) {
        r.water_box([3, 1, 0], [4, 2, 0]);
    }
    if r.open(south, EAST) {
        r.water_box([7, 1, 3], [7, 2, 4]);
    }
    if r.open(south, WEST) {
        r.water_box([0, 1, 3], [0, 2, 4]);
    }
    if r.open(north, NORTH) {
        r.water_box([3, 1, 15], [4, 2, 15]);
    }
    if r.open(north, WEST) {
        r.water_box([0, 1, 11], [0, 2, 12]);
    }
    if r.open(north, EAST) {
        r.water_box([7, 1, 11], [7, 2, 12]);
    }
}

fn entry_room<W: WorldGenVolume>(r: &mut Room<'_, '_, W>, room: u8) {
    let light = r.b.light;
    for (min, max) in [
        ([0, 3, 0], [2, 3, 7]),
        ([5, 3, 0], [7, 3, 7]),
        ([0, 2, 0], [1, 2, 7]),
        ([6, 2, 0], [7, 2, 7]),
        ([0, 1, 0], [0, 1, 7]),
        ([7, 1, 0], [7, 1, 7]),
        ([0, 1, 7], [7, 3, 7]),
        ([1, 1, 0], [2, 3, 0]),
        ([5, 1, 0], [6, 3, 0]),
    ] {
        r.solid(&light, min, max);
    }
    if r.open(room, NORTH) {
        r.water_box([3, 1, 7], [4, 2, 7]);
    }
    if r.open(room, WEST) {
        r.water_box([0, 1, 3], [1, 2, 4]);
    }
    if r.open(room, EAST) {
        r.water_box([6, 1, 3], [7, 2, 4]);
    }
}

fn penthouse<W: WorldGenVolume>(
    r: &mut Room<'_, '_, W>,
    rng: &mut XoroshiroRandom,
) {
    let (gray, light, black, lamp) = (r.b.gray, r.b.light, r.b.black, r.b.lamp);
    r.solid(&light, [2, -1, 2], [11, -1, 11]);
    r.solid(&gray, [0, -1, 0], [1, -1, 11]);
    r.solid(&gray, [12, -1, 0], [13, -1, 11]);
    r.solid(&gray, [2, -1, 0], [11, -1, 1]);
    r.solid(&gray, [2, -1, 12], [11, -1, 13]);
    r.solid(&light, [0, 0, 0], [0, 0, 13]);
    r.solid(&light, [13, 0, 0], [13, 0, 13]);
    r.solid(&light, [1, 0, 0], [12, 0, 0]);
    r.solid(&light, [1, 0, 13], [12, 0, 13]);
    for i in (2..=11).step_by(3) {
        r.place(&lamp, 0, 0, i);
        r.place(&lamp, 13, 0, i);
        r.place(&lamp, i, 0, 0);
    }
    r.solid(&light, [2, 0, 3], [4, 0, 9]);
    r.solid(&light, [9, 0, 3], [11, 0, 9]);
    r.solid(&light, [4, 0, 9], [9, 0, 11]);
    r.place(&light, 5, 0, 8);
    r.place(&light, 8, 0, 8);
    r.place(&light, 10, 0, 10);
    r.place(&light, 3, 0, 10);
    r.solid(&black, [3, 0, 3], [3, 0, 7]);
    r.solid(&black, [10, 0, 3], [10, 0, 7]);
    r.solid(&black, [6, 0, 10], [7, 0, 10]);
    for x in [3, 10] {
        for z in (2..=8).step_by(3) {
            r.solid(&light, [x, 0, z], [x, 2, z]);
        }
    }
    r.solid(&light, [5, 0, 10], [5, 2, 10]);
    r.solid(&light, [8, 0, 10], [8, 2, 10]);
    r.solid(&black, [6, -1, 7], [7, -1, 8]);
    r.water_box([6, -1, 3], [7, -1, 4]);
    r.spawn_elder(rng, 6, 1, 6);
}

fn simple_room<W: WorldGenVolume>(
    r: &mut Room<'_, '_, W>,
    rng: &mut XoroshiroRandom,
    room: u8,
    main_design: i32,
) {
    let (gray, light, black, lamp) = (r.b.gray, r.b.light, r.b.black, r.b.lamp);
    if r.above_ground_floor(room) {
        r.default_floor(0, 0, r.open(room, DOWN));
    }
    if !r.has_up(room) {
        r.box_on_fill_only([1, 4, 1], [6, 4, 6], &gray);
    }
    let openings = r
        .room(room)
        .has_opening
        .iter()
        .filter(|open| **open)
        .count();
    let centre_pillar = main_design != 0
        && rng.next_bool()
        && !r.open(room, DOWN)
        && !r.open(room, UP)
        && openings > 1;
    if main_design == 0 {
        r.solid(&light, [0, 1, 0], [2, 1, 2]);
        r.solid(&light, [0, 3, 0], [2, 3, 2]);
        r.solid(&gray, [0, 2, 0], [0, 2, 2]);
        r.solid(&gray, [1, 2, 0], [2, 2, 0]);
        r.place(&lamp, 1, 2, 1);
        r.solid(&light, [5, 1, 0], [7, 1, 2]);
        r.solid(&light, [5, 3, 0], [7, 3, 2]);
        r.solid(&gray, [7, 2, 0], [7, 2, 2]);
        r.solid(&gray, [5, 2, 0], [6, 2, 0]);
        r.place(&lamp, 6, 2, 1);
        r.solid(&light, [0, 1, 5], [2, 1, 7]);
        r.solid(&light, [0, 3, 5], [2, 3, 7]);
        r.solid(&gray, [0, 2, 5], [0, 2, 7]);
        r.solid(&gray, [1, 2, 7], [2, 2, 7]);
        r.place(&lamp, 1, 2, 6);
        r.solid(&light, [5, 1, 5], [7, 1, 7]);
        r.solid(&light, [5, 3, 5], [7, 3, 7]);
        r.solid(&gray, [7, 2, 5], [7, 2, 7]);
        r.solid(&gray, [5, 2, 7], [6, 2, 7]);
        r.place(&lamp, 6, 2, 6);
        if r.open(room, SOUTH) {
            r.solid(&light, [3, 3, 0], [4, 3, 0]);
        } else {
            r.solid(&light, [3, 3, 0], [4, 3, 1]);
            r.solid(&gray, [3, 2, 0], [4, 2, 0]);
            r.solid(&light, [3, 1, 0], [4, 1, 1]);
        }
        if r.open(room, NORTH) {
            r.solid(&light, [3, 3, 7], [4, 3, 7]);
        } else {
            r.solid(&light, [3, 3, 6], [4, 3, 7]);
            r.solid(&gray, [3, 2, 7], [4, 2, 7]);
            r.solid(&light, [3, 1, 6], [4, 1, 7]);
        }
        if r.open(room, WEST) {
            r.solid(&light, [0, 3, 3], [0, 3, 4]);
        } else {
            r.solid(&light, [0, 3, 3], [1, 3, 4]);
            r.solid(&gray, [0, 2, 3], [0, 2, 4]);
            r.solid(&light, [0, 1, 3], [1, 1, 4]);
        }
        if r.open(room, EAST) {
            r.solid(&light, [7, 3, 3], [7, 3, 4]);
        } else {
            r.solid(&light, [6, 3, 3], [7, 3, 4]);
            r.solid(&gray, [7, 2, 3], [7, 2, 4]);
            r.solid(&light, [6, 1, 3], [7, 1, 4]);
        }
    } else if main_design == 1 {
        r.solid(&light, [2, 1, 2], [2, 3, 2]);
        r.solid(&light, [2, 1, 5], [2, 3, 5]);
        r.solid(&light, [5, 1, 5], [5, 3, 5]);
        r.solid(&light, [5, 1, 2], [5, 3, 2]);
        r.place(&lamp, 2, 2, 2);
        r.place(&lamp, 2, 2, 5);
        r.place(&lamp, 5, 2, 5);
        r.place(&lamp, 5, 2, 2);
        r.solid(&light, [0, 1, 0], [1, 3, 0]);
        r.solid(&light, [0, 1, 1], [0, 3, 1]);
        r.solid(&light, [0, 1, 7], [1, 3, 7]);
        r.solid(&light, [0, 1, 6], [0, 3, 6]);
        r.solid(&light, [6, 1, 7], [7, 3, 7]);
        r.solid(&light, [7, 1, 6], [7, 3, 6]);
        r.solid(&light, [6, 1, 0], [7, 3, 0]);
        r.solid(&light, [7, 1, 1], [7, 3, 1]);
        r.place(&gray, 1, 2, 0);
        r.place(&gray, 0, 2, 1);
        r.place(&gray, 1, 2, 7);
        r.place(&gray, 0, 2, 6);
        r.place(&gray, 6, 2, 7);
        r.place(&gray, 7, 2, 6);
        r.place(&gray, 6, 2, 0);
        r.place(&gray, 7, 2, 1);
        if !r.open(room, SOUTH) {
            r.solid(&light, [1, 3, 0], [6, 3, 0]);
            r.solid(&gray, [1, 2, 0], [6, 2, 0]);
            r.solid(&light, [1, 1, 0], [6, 1, 0]);
        }
        if !r.open(room, NORTH) {
            r.solid(&light, [1, 3, 7], [6, 3, 7]);
            r.solid(&gray, [1, 2, 7], [6, 2, 7]);
            r.solid(&light, [1, 1, 7], [6, 1, 7]);
        }
        if !r.open(room, WEST) {
            r.solid(&light, [0, 3, 1], [0, 3, 6]);
            r.solid(&gray, [0, 2, 1], [0, 2, 6]);
            r.solid(&light, [0, 1, 1], [0, 1, 6]);
        }
        if !r.open(room, EAST) {
            r.solid(&light, [7, 3, 1], [7, 3, 6]);
            r.solid(&gray, [7, 2, 1], [7, 2, 6]);
            r.solid(&light, [7, 1, 1], [7, 1, 6]);
        }
    } else if main_design == 2 {
        r.solid(&light, [0, 1, 0], [0, 1, 7]);
        r.solid(&light, [7, 1, 0], [7, 1, 7]);
        r.solid(&light, [1, 1, 0], [6, 1, 0]);
        r.solid(&light, [1, 1, 7], [6, 1, 7]);
        r.solid(&black, [0, 2, 0], [0, 2, 7]);
        r.solid(&black, [7, 2, 0], [7, 2, 7]);
        r.solid(&black, [1, 2, 0], [6, 2, 0]);
        r.solid(&black, [1, 2, 7], [6, 2, 7]);
        r.solid(&light, [0, 3, 0], [0, 3, 7]);
        r.solid(&light, [7, 3, 0], [7, 3, 7]);
        r.solid(&light, [1, 3, 0], [6, 3, 0]);
        r.solid(&light, [1, 3, 7], [6, 3, 7]);
        r.solid(&black, [0, 1, 3], [0, 2, 4]);
        r.solid(&black, [7, 1, 3], [7, 2, 4]);
        r.solid(&black, [3, 1, 0], [4, 2, 0]);
        r.solid(&black, [3, 1, 7], [4, 2, 7]);
        if r.open(room, SOUTH) {
            r.water_box([3, 1, 0], [4, 2, 0]);
        }
        if r.open(room, NORTH) {
            r.water_box([3, 1, 7], [4, 2, 7]);
        }
        if r.open(room, WEST) {
            r.water_box([0, 1, 3], [0, 2, 4]);
        }
        if r.open(room, EAST) {
            r.water_box([7, 1, 3], [7, 2, 4]);
        }
    }
    if centre_pillar {
        r.solid(&light, [3, 1, 3], [4, 1, 4]);
        r.solid(&gray, [3, 2, 3], [4, 2, 4]);
        r.solid(&light, [3, 3, 3], [4, 3, 4]);
    }
}

fn simple_top_room<W: WorldGenVolume>(
    r: &mut Room<'_, '_, W>,
    rng: &mut XoroshiroRandom,
    room: u8,
) {
    let (gray, light, black, wet_sponge) = (r.b.gray, r.b.light, r.b.black, r.b.wet_sponge);
    if r.above_ground_floor(room) {
        r.default_floor(0, 0, r.open(room, DOWN));
    }
    if !r.has_up(room) {
        r.box_on_fill_only([1, 4, 1], [6, 4, 6], &gray);
    }
    for x in 1..=6 {
        for z in 1..=6 {
            if rng.next_i32_bound(3) != 0 {
                let y0 = 2 + if rng.next_i32_bound(4) == 0 { 0 } else { 1 };
                r.solid(&wet_sponge, [x, y0, z], [x, 3, z]);
            }
        }
    }
    r.solid(&light, [0, 1, 0], [0, 1, 7]);
    r.solid(&light, [7, 1, 0], [7, 1, 7]);
    r.solid(&light, [1, 1, 0], [6, 1, 0]);
    r.solid(&light, [1, 1, 7], [6, 1, 7]);
    r.solid(&black, [0, 2, 0], [0, 2, 7]);
    r.solid(&black, [7, 2, 0], [7, 2, 7]);
    r.solid(&black, [1, 2, 0], [6, 2, 0]);
    r.solid(&black, [1, 2, 7], [6, 2, 7]);
    r.solid(&light, [0, 3, 0], [0, 3, 7]);
    r.solid(&light, [7, 3, 0], [7, 3, 7]);
    r.solid(&light, [1, 3, 0], [6, 3, 0]);
    r.solid(&light, [1, 3, 7], [6, 3, 7]);
    r.solid(&black, [0, 1, 3], [0, 2, 4]);
    r.solid(&black, [7, 1, 3], [7, 2, 4]);
    r.solid(&black, [3, 1, 0], [4, 2, 0]);
    r.solid(&black, [3, 1, 7], [4, 2, 7]);
    if r.open(room, SOUTH) {
        r.water_box([3, 1, 0], [4, 2, 0]);
    }
}

fn wing_room<W: WorldGenVolume>(
    r: &mut Room<'_, '_, W>,
    rng: &mut XoroshiroRandom,
    main_design: i32,
) {
    let (light, black, lamp) = (r.b.light, r.b.black, r.b.lamp);
    if main_design == 0 {
        for i in 0..4 {
            r.solid(&light, [10 - i, 3 - i, 20 - i], [12 + i, 3 - i, 20]);
        }
        r.solid(&light, [7, 0, 6], [15, 0, 16]);
        r.solid(&light, [6, 0, 6], [6, 3, 20]);
        r.solid(&light, [16, 0, 6], [16, 3, 20]);
        r.solid(&light, [7, 1, 7], [7, 1, 20]);
        r.solid(&light, [15, 1, 7], [15, 1, 20]);
        r.solid(&light, [7, 1, 6], [9, 3, 6]);
        r.solid(&light, [13, 1, 6], [15, 3, 6]);
        r.solid(&light, [8, 1, 7], [9, 1, 7]);
        r.solid(&light, [13, 1, 7], [14, 1, 7]);
        r.solid(&light, [9, 0, 5], [13, 0, 5]);
        r.solid(&black, [10, 0, 7], [12, 0, 7]);
        r.solid(&black, [8, 0, 10], [8, 0, 12]);
        r.solid(&black, [14, 0, 10], [14, 0, 12]);
        for z in (7..=18).rev().step_by(3) {
            r.place(&lamp, 6, 3, z);
            r.place(&lamp, 16, 3, z);
        }
        r.place(&lamp, 10, 0, 10);
        r.place(&lamp, 12, 0, 10);
        r.place(&lamp, 10, 0, 12);
        r.place(&lamp, 12, 0, 12);
        r.place(&lamp, 8, 3, 6);
        r.place(&lamp, 14, 3, 6);
        r.place(&light, 4, 2, 4);
        r.place(&lamp, 4, 1, 4);
        r.place(&light, 4, 0, 4);
        r.place(&light, 18, 2, 4);
        r.place(&lamp, 18, 1, 4);
        r.place(&light, 18, 0, 4);
        r.place(&light, 4, 2, 18);
        r.place(&lamp, 4, 1, 18);
        r.place(&light, 4, 0, 18);
        r.place(&light, 18, 2, 18);
        r.place(&lamp, 18, 1, 18);
        r.place(&light, 18, 0, 18);
        r.place(&light, 9, 7, 20);
        r.place(&light, 13, 7, 20);
        r.solid(&light, [6, 0, 21], [7, 4, 21]);
        r.solid(&light, [15, 0, 21], [16, 4, 21]);
        r.spawn_elder(rng, 11, 2, 16);
    } else if main_design == 1 {
        r.solid(&light, [9, 3, 18], [13, 3, 20]);
        r.solid(&light, [9, 0, 18], [9, 2, 18]);
        r.solid(&light, [13, 0, 18], [13, 2, 18]);
        for x in [9, 13] {
            r.place(&light, x, 6, 20);
            r.place(&lamp, x, 5, 20);
            r.place(&light, x, 4, 20);
        }
        r.solid(&light, [7, 3, 7], [15, 3, 14]);
        for x in [10, 12] {
            r.solid(&light, [x, 0, 10], [x, 6, 10]);
            r.solid(&light, [x, 0, 12], [x, 6, 12]);
            r.place(&lamp, x, 0, 10);
            r.place(&lamp, x, 0, 12);
            r.place(&lamp, x, 4, 10);
            r.place(&lamp, x, 4, 12);
        }
        for x in [8, 14] {
            r.solid(&light, [x, 0, 7], [x, 2, 7]);
            r.solid(&light, [x, 0, 14], [x, 2, 14]);
        }
        r.solid(&black, [8, 3, 8], [8, 3, 13]);
        r.solid(&black, [14, 3, 8], [14, 3, 13]);
        r.spawn_elder(rng, 11, 5, 13);
    }
}
