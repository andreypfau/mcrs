//! Cave culling: which sections a sight line can actually reach from the camera.
//!
//! Once a frame a breadth-first walk starts at the camera's section and steps into a neighbour only
//! when three tests pass: the section being left can be crossed from the entry face to the exit face
//! (the mask the mesher baked), the step does not reverse an axis already spent, and the neighbour's
//! box lands inside the frustum. Everything the walk never reaches sits behind rock and is thrown
//! away before any vertex work.

use bevy::camera::primitives::{Aabb, Frustum};
use bevy::prelude::*;

use crate::anvil::SECTION_SIZE;
use crate::pack::RegionGrid;

/// Seeds and the camera's own section fan out through all six faces; there is no real face 7.
const ENTRY_ANY: u32 = 7;

/// Entry faces a section can be reached through: the six real ones, and [`ENTRY_ANY`] at 7.
const ENTRIES: usize = 8;

/// No sight line has entered this section through this face yet. Real values are six bits of spent
/// directions, so the high bits are free for the marker.
const NEVER: u8 = u8::MAX;

/// Queue entries carry the section slot, the face the walk arrived through, and the directions
/// already spent. Twenty bits of slot cover a world of a million sections, which is far past what
/// the loader can hold.
const QUEUE_SLOT_BITS: u32 = 20;
const QUEUE_ENTRY_SHIFT: u32 = QUEUE_SLOT_BITS;
const QUEUE_DIRS_SHIFT: u32 = QUEUE_ENTRY_SHIFT + 3;

/// `FACE_AXES` order: 0 Down(−Y), 1 Up(+Y), 2 North(−Z), 3 South(+Z), 4 West(−X), 5 East(+X).
const NEIGHBOUR: [[i32; 3]; 6] =
    [[0, -1, 0], [0, 1, 0], [0, 0, -1], [0, 0, 1], [-1, 0, 0], [1, 0, 0]];

/// The negative face of each axis, so that `AXIS_FACE[axis] | positive as u32` is the direction.
const AXIS_FACE: [u32; 3] = [4, 0, 2];

#[derive(Resource)]
pub struct CaveCull {
    pub enabled: bool,
    /// Reached sections, uploaded to the GPU as they are. A section that fails the frustum is marked
    /// here and never expanded; the GPU drops it anyway with its own frustum test.
    pub bits: Box<[u32]>,
    /// Sections whose box the frustum accepted, so the test is paid once however many faces the
    /// walk later arrives through.
    inside: Box<[u32]>,
    /// Spent directions per section and entry face, indexed `slot << 3 | entry`, or [`NEVER`].
    /// Which exits a section opens depends on the face entered through, so a walk that has already
    /// crossed it one way still has to cross it the other.
    spent: Vec<u8>,
    /// [`mesh::RegionMesh::connectivity`](crate::mesh::RegionMesh::connectivity), bit
    /// `entry * 6 + exit`.
    conn: Vec<u64>,
    grid: RegionGrid,
    /// The sections the loaded world really holds, which is what the walk may step onto. The grid
    /// rounds up to whole regions, so its far corner can be padding that no section occupies.
    sections: [usize; 3],
    /// Section coordinates of the loaded window's corner. Region coordinates run either side of
    /// zero, so this is signed on all three axes and not only the vertical one.
    min_section: [i32; 3],
    /// One push per section and entry face, plus one more each time a later arrival through that
    /// face turns out to have spent fewer directions. `dirs` only ever shrinks, so those repeats
    /// are bounded by its six bits.
    queue: Vec<u32>,
}

impl CaveCull {
    pub fn new(
        conn: Vec<u64>,
        grid: RegionGrid,
        sections: [usize; 3],
        min_section: [i32; 3],
    ) -> Self {
        let covers = RegionGrid::covering(sections);
        assert!(
            covers == grid,
            "the walk was given a {grid:?} grid for a world of {sections:?} sections, which needs \
             {covers:?}"
        );
        let slots = grid.slots();
        assert!(
            slots < 1 << QUEUE_SLOT_BITS,
            "the walk queue carries a section slot in {QUEUE_SLOT_BITS} bits, and this world has \
             {slots} of them"
        );
        Self {
            enabled: true,
            bits: vec![u32::MAX; slots.div_ceil(32)].into_boxed_slice(),
            inside: vec![0; slots.div_ceil(32)].into_boxed_slice(),
            spent: vec![NEVER; slots * ENTRIES],
            conn,
            grid,
            sections,
            min_section,
            queue: Vec::with_capacity(sections[0] * sections[1] * sections[2]),
        }
    }

    /// Words of the bitset, which is also how much of it goes to the GPU.
    pub fn words(&self) -> usize {
        self.bits.len()
    }

    pub fn reached(&self) -> u32 {
        self.bits.iter().map(|word| word.count_ones()).sum()
    }

    fn run(&mut self, camera: Vec3, frustum: &Frustum) {
        self.bits.fill(0);
        self.inside.fill(0);
        self.spent.fill(NEVER);
        self.queue.clear();
        if !self.seed(camera, frustum) {
            self.bits.fill(u32::MAX);
            return;
        }

        let hi = self.limit();
        let mut head = 0;
        while head < self.queue.len() {
            let node = self.queue[head];
            head += 1;
            let slot = node & ((1 << QUEUE_SLOT_BITS) - 1);
            let entry = (node >> QUEUE_ENTRY_SHIFT) & 7;
            let dirs = (node >> QUEUE_DIRS_SHIFT) & 0x3f;
            let here = self.grid.section_at(slot as usize).map(|n| n as i32);
            let mask = self.conn[slot as usize];

            for exit in 0..6u32 {
                // 1. no reversals: the far side of a direction already spent.
                if dirs & (1 << (exit ^ 1)) != 0 {
                    continue;
                }
                // 2. connectivity. `exit == entry` never gets here: that is exactly the reversal of
                //    the step that set `entry`, and `dirs` already rejected it.
                if entry != ENTRY_ANY && mask >> (entry * 6 + exit) & 1 == 0 {
                    continue;
                }
                let step = NEIGHBOUR[exit as usize];
                let next = [here[0] + step[0], here[1] + step[1], here[2] + step[2]];
                // Against what the world really holds, not what the region grid spans: the grid
                // rounds up to whole regions, so its far corner can be slots no section occupies.
                if (0..3).any(|a| next[a] < 0 || next[a] > hi[a]) {
                    continue;
                }
                let neighbour =
                    self.grid.slot(next[0] as usize, next[1] as usize, next[2] as usize) as u32;
                // 3. the frustum, inside `push`, at most once per section.
                self.push(neighbour, exit ^ 1, dirs | 1 << exit, frustum);
            }
        }
    }

    /// `false` asks the caller to give up for this frame: with the camera inside a section no sight
    /// line crosses, the walk would die on its six neighbours and leave an empty screen.
    fn seed(&mut self, camera: Vec3, frustum: &Frustum) -> bool {
        let hi = self.limit();
        let size = SECTION_SIZE as f32;
        let cs = [
            camera.x.div_euclid(size) as i32 - self.min_section[0],
            camera.y.div_euclid(size) as i32 - self.min_section[1],
            camera.z.div_euclid(size) as i32 - self.min_section[2],
        ];

        // Travelling to the region already spends every axis the camera is outside of, so reversing
        // along one is forbidden for any path that starts on the boundary.
        let mut dirs = 0u32;
        let mut outside = [0i32; 3];
        for a in 0..3 {
            if cs[a] < 0 {
                outside[a] = -1;
                dirs |= 1 << (AXIS_FACE[a] | 1);
            } else if cs[a] > hi[a] {
                outside[a] = 1;
                dirs |= 1 << AXIS_FACE[a];
            }
        }

        if outside == [0, 0, 0] {
            let slot = self.grid.slot(cs[0] as usize, cs[1] as usize, cs[2] as usize) as u32;
            // ponytail: a camera in solid rock or a sealed cavern turns culling off for the whole
            // frame instead of honestly culling from inside that cavern. The ceiling is that an
            // orbit pulled down to its minimum radius inside the terrain stops saving anything.
            // The upgrade is a flood fill from the camera's position within the section rather than
            // the mask of the section as a whole.
            if self.conn[slot as usize] == 0 {
                return false;
            }
            self.push(slot, ENTRY_ANY, 0, frustum);
            return true;
        }

        for a in 0..3 {
            if outside[a] == 0 {
                continue;
            }
            let (b, c) = ((a + 1) % 3, (a + 2) % 3);
            let fixed = if outside[a] < 0 { 0 } else { hi[a] };
            for u in 0..=hi[b] {
                for v in 0..=hi[c] {
                    let mut p = [0i32; 3];
                    p[a] = fixed;
                    p[b] = u;
                    p[c] = v;
                    let slot =
                        self.grid.slot(p[0] as usize, p[1] as usize, p[2] as usize) as u32;
                    // ponytail: `entry = ANY` lets a seed tunnel exactly one section into the
                    // boundary rock. The ceiling is that the region's outer shell on the camera's
                    // side is effectively never culled. It costs nothing: a fully buried section
                    // emits no quad group at all, so there is nothing there to draw.
                    self.push(slot, ENTRY_ANY, dirs, frustum);
                }
            }
        }
        true
    }

    fn push(&mut self, slot: u32, entry: u32, dirs: u32, frustum: &Frustum) {
        // Two arrivals through the same face merge by intersection rather than union. `dirs` is a
        // set of spent directions and it works by forbidding, so the smaller set is the weaker
        // filter: dropping to it can only open exits, never close one that was already taken.
        let seen = &mut self.spent[(slot as usize) << 3 | entry as usize];
        let merged = if *seen == NEVER { dirs as u8 } else { *seen & dirs as u8 };
        if merged == *seen {
            return;
        }
        *seen = merged;

        // Marked before the frustum test so each section is tested at most once per frame. A section
        // the frustum rejects stays in the bitset — `in_frustum` in the shader drops it anyway — but
        // is never expanded, so it cannot carry a sight line around an obstacle.
        let (word, bit) = ((slot >> 5) as usize, 1u32 << (slot & 31));
        if self.bits[word] & bit == 0 {
            self.bits[word] |= bit;
            if frustum.intersects_obb_identity(&self.aabb(slot)) {
                self.inside[word] |= bit;
            }
        }
        if self.inside[word] & bit == 0 {
            return;
        }
        self.queue
            .push(slot | entry << QUEUE_ENTRY_SHIFT | (merged as u32) << QUEUE_DIRS_SHIFT);
    }

    /// The last section on each axis the walk may step onto.
    fn limit(&self) -> [i32; 3] {
        [
            self.sections[0] as i32 - 1,
            self.sections[1] as i32 - 1,
            self.sections[2] as i32 - 1,
        ]
    }

    fn aabb(&self, slot: u32) -> Aabb {
        let [sx, sy, sz] = self.grid.section_at(slot as usize).map(|n| n as i32);
        let size = SECTION_SIZE as f32;
        let min = Vec3::new(
            (sx + self.min_section[0]) as f32 * size,
            (sy + self.min_section[1]) as f32 * size,
            (sz + self.min_section[2]) as f32 * size,
        );
        Aabb::from_min_max(min, min + size)
    }
}

pub fn cave_cull(
    mut cave: ResMut<CaveCull>,
    camera: Single<(&GlobalTransform, &Frustum), With<Camera3d>>,
) {
    let (transform, frustum) = *camera;
    if !cave.enabled {
        cave.bits.fill(u32::MAX);
        return;
    }
    cave.run(transform.translation(), frustum);
}

/// Press C to turn the walk off and on. The shader has no flag for it: switched off, a bitset of all
/// ones goes up, so both states run through exactly the same code.
pub fn toggle(keys: Res<ButtonInput<KeyCode>>, mut cave: ResMut<CaveCull>) {
    if keys.just_pressed(KeyCode::KeyC) {
        cave.enabled = !cave.enabled;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mesh::CONNECT_ALL;
    use crate::anvil::REGION_CHUNKS;
    use bevy::camera::CameraProjection;

    /// A shallow world, so a fixture stays small while still spanning several render regions.
    const SECTIONS: [usize; 3] = [REGION_CHUNKS, 4, REGION_CHUNKS];

    fn grid() -> RegionGrid {
        RegionGrid::covering(SECTIONS)
    }

    fn slot(sx: usize, sy: usize, sz: usize) -> usize {
        grid().slot(sx, sy, sz)
    }

    fn walk(conn: Vec<u64>) -> CaveCull {
        CaveCull::new(conn, grid(), SECTIONS, [0; 3])
    }

    fn empty_conn() -> Vec<u64> {
        vec![0u64; grid().slots()]
    }

    fn open_conn() -> Vec<u64> {
        vec![CONNECT_ALL; grid().slots()]
    }

    /// A very wide frustum: these tests measure connectivity and the no-reversal rule, not Bevy's
    /// own code.
    fn wide(eye: Vec3, at: Vec3) -> Frustum {
        PerspectiveProjection {
            fov: 2.8,
            far: 4000.0,
            ..default()
        }
        .compute_frustum(&GlobalTransform::from(
            Transform::from_translation(eye).looking_at(at, Vec3::Y),
        ))
    }

    fn visible(cave: &CaveCull, sx: usize, sy: usize, sz: usize) -> bool {
        let slot = slot(sx, sy, sz);
        cave.bits[slot >> 5] >> (slot & 31) & 1 != 0
    }

    /// The plane seed, the connectivity filter and the no-reversal rule all have to be right at once
    /// for this to pass.
    #[test]
    fn a_solid_wall_hides_what_is_behind_it() {
        let mut conn = open_conn();
        for sy in 0..SECTIONS[1] {
            for sz in 0..SECTIONS[2] {
                conn[slot(20, sy, sz)] = 0;
            }
        }
        let mut cave = walk(conn);
        let eye = Vec3::new(900.0, 32.0, 256.0);
        cave.run(eye, &wide(eye, Vec3::new(0.0, 32.0, 256.0)));
        assert!(visible(&cave, 25, 2, 16), "section in front of the wall");
        assert!(!visible(&cave, 10, 2, 16), "section behind the wall");
    }

    /// Region coordinates run either side of zero, so the window a walk covers can start at a
    /// negative section. Losing that offset puts the camera outside the window instead of inside
    /// it, and the walk then seeds the far boundary and reports the wall's other side.
    #[test]
    fn a_window_below_the_origin_culls_from_where_the_camera_really_is() {
        let corner = [-(REGION_CHUNKS as i32), 0, -(REGION_CHUNKS as i32)];
        let mut conn = open_conn();
        for sy in 0..SECTIONS[1] {
            for sz in 0..SECTIONS[2] {
                conn[slot(20, sy, sz)] = 0;
            }
        }
        let mut cave = CaveCull::new(conn, grid(), SECTIONS, corner);
        // Window-relative section (25, 2, 16), which with this corner is world section (-7, 2, -16).
        let eye = Vec3::new(-104.0, 40.0, -248.0);
        cave.run(eye, &wide(eye, Vec3::new(-900.0, 40.0, -248.0)));
        assert!(visible(&cave, 25, 2, 16), "the section the camera stands in");
        assert!(!visible(&cave, 10, 2, 16), "section behind the wall");
    }

    /// An asymmetric fixture: exactly one pair of faces is open, North↔West. A walk entering from
    /// the north has to leave to the west and nowhere else, which is what pins down the bit order.
    #[test]
    fn a_corner_section_turns_only_the_way_its_mask_allows() {
        let mut conn = empty_conn();
        conn[slot(16, 2, 15)] = CONNECT_ALL;
        conn[slot(16, 2, 16)] = 1 << (2 * 6 + 4) | 1 << (4 * 6 + 2);
        let mut cave = walk(conn);
        let eye = Vec3::new(264.0, 40.0, 248.0);
        cave.run(eye, &wide(eye, Vec3::new(264.0, 40.0, 400.0)));
        assert!(visible(&cave, 15, 2, 16), "the turn to the west");
        assert!(!visible(&cave, 17, 2, 16), "east is closed by the mask");
        assert!(!visible(&cave, 16, 3, 16), "up is closed by the mask");
    }

    fn pair(entry: u32, exit: u32) -> u64 {
        1 << (entry * 6 + exit)
    }

    /// One section crossable West→Up and North→Down, with a corridor reaching it from the west and
    /// another from the north. The longer corridor delivers its sight line second.
    fn two_ways_in(mx: usize, mz: usize) -> CaveCull {
        let mut conn = empty_conn();
        conn[slot(mx, 2, mz)] = pair(4, 1) | pair(1, 4) | pair(2, 0) | pair(0, 2);
        for x in 0..mx {
            conn[slot(x, 2, mz)] = pair(4, 5) | pair(5, 4);
        }
        for z in 0..mz {
            conn[slot(mx, 2, z)] = pair(2, 3) | pair(3, 2);
        }
        walk(conn)
    }

    /// Which exits a section opens depends on the face the sight line came in through, so a section
    /// already crossed one way still has to be crossed the other. Marking it done on the first
    /// arrival drops the second way in along with everything only it could see, and which of the two
    /// survives comes down to the order the walk happened to reach them in.
    #[test]
    fn a_section_reached_twice_opens_the_exits_of_both_ways_in() {
        for (mx, mz) in [(16, 24), (24, 16)] {
            let mut cave = two_ways_in(mx, mz);
            let eye = Vec3::new(-160.0, 40.0, -160.0);
            cave.run(eye, &wide(eye, Vec3::new(400.0, 40.0, 400.0)));
            assert!(visible(&cave, mx, 3, mz), "{mx},{mz}: the way up, entered from the west");
            assert!(visible(&cave, mx, 1, mz), "{mx},{mz}: the way down, entered from the north");
        }
    }

    /// A camera inside rock has nothing to start from; culling has to switch itself off for the
    /// frame rather than leave an empty screen.
    #[test]
    fn a_camera_sealed_in_rock_gives_up_instead_of_culling_everything() {
        let mut cave = walk(empty_conn());
        let eye = Vec3::new(264.0, 40.0, 264.0);
        cave.run(eye, &wide(eye, Vec3::new(264.0, 40.0, 400.0)));
        assert_eq!(cave.reached(), cave.words() as u32 * 32);
    }
}
