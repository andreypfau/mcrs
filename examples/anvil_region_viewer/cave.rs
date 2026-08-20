//! Cave culling: which sections a sight line can actually reach from the camera.
//!
//! Once a frame a breadth-first walk starts at the camera's section and steps into a neighbour only
//! when four tests pass: the section being left can be crossed from the entry face to the exit face
//! (the mask the mesher baked), the step does not reverse an axis already spent, the arrival says
//! something the same face has not already said, and the neighbour's box lands inside the frustum.
//! Everything the walk never reaches — behind rock, off the screen, or past the loaded world — is
//! thrown away before any vertex work.

use bevy::camera::primitives::{Aabb, Frustum};
use bevy::prelude::*;

use crate::anvil::SECTION_SIZE;
use crate::pack::{RegionGrid, SECTIONS_PER_RENDER_REGION};

/// Which way each face steps, built from the mesher's own table rather than written out again: the
/// crossing masks the walk reads are indexed by that same face order, and a second table would have
/// to be kept agreeing with it by hand. Built at compile time, because the walk reads it six times
/// per section and calling through costs more than the whole rest of the step.
const NEIGHBOUR: [[i32; 3]; 6] = [
    crate::mesh::face_normal(0),
    crate::mesh::face_normal(1),
    crate::mesh::face_normal(2),
    crate::mesh::face_normal(3),
    crate::mesh::face_normal(4),
    crate::mesh::face_normal(5),
];

/// Seeds and the camera's own section fan out through all six faces; there is no real face 6.
const ENTRY_ANY: u32 = 6;

/// Entry faces a section can be reached through: the six real ones, and [`ENTRY_ANY`] just past
/// them. Seven rather than a round eight because the marks are cleared in full every frame, and
/// an eighth that nothing ever writes would be an eighth of that clearing spent on nothing.
const ENTRIES: usize = 7;

/// No sight line has entered this section through this face yet. Real values are six bits of spent
/// directions, so the high bits are free for the marker.
const NEVER: u8 = u8::MAX;

/// Queue entries carry the section slot, the face the walk arrived through, and the directions
/// already spent. Twenty bits of slot cover a world of a million sections, which is far past what
/// the loader can hold.
const QUEUE_SLOT_BITS: u32 = 20;
const QUEUE_ENTRY_SHIFT: u32 = QUEUE_SLOT_BITS;
const QUEUE_DIRS_SHIFT: u32 = QUEUE_ENTRY_SHIFT + 3;

/// The tail of always-set slots starts at a whole word, which only holds while a render region is
/// a whole number of words of sections.
const _: () = assert!(SECTIONS_PER_RENDER_REGION % 32 == 0);

/// A queue entry has to fit the word it is stored in.
const _: () = assert!(QUEUE_DIRS_SHIFT + 6 <= 32);

/// Two things the walk rests on that the face order could quietly take away: that a face and its
/// opposite differ by one bit, which is the whole of the no-reversal rule, and that [`AXIS_FACE`]
/// names each axis's negative face, which is how a seed outside the grid decides what it has spent
/// getting there. Neither would fail loudly if the mesher reordered its faces.
const _: () = {
    let mut face = 0;
    while face < 6 {
        let there = crate::mesh::face_normal(face);
        let back = crate::mesh::face_normal(face ^ 1);
        assert!(there[0] == -back[0] && there[1] == -back[1] && there[2] == -back[2]);
        face += 1;
    }
    let mut axis = 0;
    while axis < 3 {
        assert!(crate::mesh::face_normal(AXIS_FACE[axis] as usize)[axis] == -1);
        axis += 1;
    }
};

/// The negative face of each axis, so that `AXIS_FACE[axis] | positive as u32` is the direction.
const AXIS_FACE: [u32; 3] = [4, 0, 2];

/// Render regions the walk covers on each horizontal axis.
///
/// The walk's cost and its arrays follow the camera rather than the loaded window: a window can hold
/// sixty-four region files, eight to a side, and a grid spanning that would be a million and a half
/// sections for the walk to clear every frame — past what a queue entry can even name. Six render regions
/// is fifteen hundred blocks across, which comfortably covers what the memory budget can hold
/// around the camera.
pub const CAVE_REGIONS: usize = 6;

/// The grid the walk covers. Its vertical extent is the world's whole height, because a sight line
/// goes up and down as readily as sideways.
pub fn cave_grid() -> RegionGrid {
    RegionGrid {
        x: CAVE_REGIONS,
        y: crate::anvil::SECTIONS_Y / crate::pack::RENDER_REGION_Y,
        z: CAVE_REGIONS,
    }
}

#[derive(Resource)]
pub struct CaveCull {
    pub enabled: bool,
    /// Reached sections, uploaded to the GPU as they are. A section that fails the frustum is marked
    /// here and never expanded; the GPU drops it anyway with its own frustum test.
    ///
    /// One render region's worth of slots past the grid is kept permanently set. A region the
    /// walk's grid does not reach points its draws there, so geometry outside the walk is drawn
    /// rather than culled by a bit that was never written.
    pub bits: Box<[u32]>,
    /// Sections whose box the frustum accepted, so the test is paid once however many faces the
    /// walk later arrives through.
    inside: Box<[u32]>,
    /// Spent directions per section and entry face, indexed `slot * ENTRIES + entry`, or
    /// [`NEVER`].
    /// Which exits a section opens depends on the face entered through, so a walk that has already
    /// crossed it one way still has to cross it the other.
    spent: Vec<u8>,
    /// Per-section crossing masks, bit `entry * 6 + exit`. A slot no loaded section covers stays
    /// [`CONNECT_ALL`](crate::mesh::CONNECT_ALL): unloaded is not the same as solid, and a walk
    /// that treated it as solid would cull away geometry that is still there.
    conn: Vec<u64>,
    grid: RegionGrid,
    /// Section coordinates of the walk's own corner. It slides with the camera, so this is the one
    /// piece of the walk that moves.
    min_section: [i32; 3],
    /// The loaded world, in world section coordinates. The walk may not step outside it: the grid
    /// can reach past the last file, and a sight line let loose in that empty space would travel
    /// round the outside of the world and light up its far side.
    world_min: [i32; 3],
    world_extent: [usize; 3],
    /// One push per section and entry face, plus one more each time a later arrival through that
    /// face turns out to have spent fewer directions. `dirs` only ever shrinks, so those repeats
    /// are bounded by its six bits.
    queue: Vec<u32>,
    /// Microseconds the last walks took. The walk is the most expensive thing the main thread does
    /// each frame, and nothing else reports what a system costs, so it reports its own.
    took: Box<[u32; CaveCull::TIMED]>,
    walks: usize,
}

impl CaveCull {
    pub fn new(grid: RegionGrid, min_section: [i32; 3], world_extent: [usize; 3]) -> Self {
        let slots = grid.slots();
        assert!(
            slots + SECTIONS_PER_RENDER_REGION < 1 << QUEUE_SLOT_BITS,
            "the walk queue carries a section slot in {QUEUE_SLOT_BITS} bits, and this grid has \
             {slots} of them"
        );
        let words = (slots + SECTIONS_PER_RENDER_REGION).div_ceil(32);
        Self {
            // The walk costs the main thread more than any other system, so a measurement of
            // anything else needs to be able to take it out from a terminal.
            enabled: !std::env::var("ANVIL_CAVE").is_ok_and(|on| on == "0"),
            bits: vec![u32::MAX; words].into_boxed_slice(),
            inside: vec![0; words].into_boxed_slice(),
            spent: vec![NEVER; slots * ENTRIES],
            conn: vec![crate::mesh::CONNECT_ALL; slots],
            grid,
            min_section,
            world_min: min_section,
            world_extent,
            queue: Vec::with_capacity(slots),
            took: Box::new([0; Self::TIMED]),
            walks: 0,
        }
    }

    /// Walks the median is taken over, which at these frame rates is well under a second.
    const TIMED: usize = 256;

    /// The median of the walks held, in milliseconds, or nothing before the first one. A median
    /// because one walk that lands on a page fault should not move a figure describing the rest.
    pub fn took_ms(&self) -> Option<f32> {
        let held = self.walks.min(Self::TIMED);
        if held == 0 {
            return None;
        }
        let mut sorted = [0u32; Self::TIMED];
        sorted[..held].copy_from_slice(&self.took[..held]);
        sorted[..held].sort_unstable();
        Some(sorted[held / 2] as f32 / 1000.0)
    }

    pub fn grid(&self) -> RegionGrid {
        self.grid
    }

    pub fn min_section(&self) -> [i32; 3] {
        self.min_section
    }

    /// Where the always-set run past the grid begins, which is where a region the walk does not
    /// reach points its draws. See [`CaveCull::bits`].
    pub fn always_visible(&self) -> u32 {
        self.grid.slots() as u32
    }

    /// Moves the walk's grid to a new corner, forgetting everything it held. The caller writes
    /// back the masks of whatever is still loaded inside it.
    pub fn retarget(&mut self, min_section: [i32; 3]) {
        self.min_section = min_section;
        self.conn.fill(crate::mesh::CONNECT_ALL);
    }

    /// Takes the crossing masks of a render region the mesher has just finished, by the section's
    /// place inside that region. Sections it does not name keep what they had.
    pub fn set_region(&mut self, base: usize, entries: &[(u32, u64)]) {
        debug_assert_eq!(base % SECTIONS_PER_RENDER_REGION, 0, "a region starts where a region starts");
        for &(local, mask) in entries {
            self.conn[base + local as usize] = mask;
        }
    }

    /// Forgets what a render region's sections said.
    pub fn forget(&mut self, base: usize) {
        debug_assert_eq!(base % SECTIONS_PER_RENDER_REGION, 0, "a region starts where a region starts");
        self.conn[base..base + SECTIONS_PER_RENDER_REGION].fill(crate::mesh::CONNECT_ALL);
    }

    /// Words of the bitset, which is also how much of it goes to the GPU.
    pub fn words(&self) -> usize {
        self.bits.len()
    }

    pub fn reached(&self) -> u32 {
        self.bits.iter().map(|word| word.count_ones()).sum()
    }

    fn run(&mut self, camera: Vec3, frustum: &Frustum) {
        self.spent.fill(NEVER);
        self.bits.fill(0);
        self.open_the_tail();
        self.inside.fill(0);
        self.queue.clear();
        let (lo, hi) = self.bounds();
        if !self.seed(camera, frustum, lo, hi) {
            self.bits.fill(u32::MAX);
            return;
        }

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
                if (0..3).any(|a| next[a] < lo[a] || next[a] > hi[a]) {
                    continue;
                }
                let neighbour =
                    self.grid.slot(next[0] as usize, next[1] as usize, next[2] as usize) as u32;
                // 3 and 4, both inside `push`: whether this arrival says anything the same face
                //    has not already said, and then the frustum, at most once per section.
                self.push(neighbour, exit ^ 1, dirs | 1 << exit, frustum);
            }
        }
    }

    /// `false` asks the caller to give up for this frame: with the camera inside a section no sight
    /// line crosses, the walk would die on its six neighbours and leave an empty screen.
    fn seed(&mut self, camera: Vec3, frustum: &Frustum, lo: [i32; 3], hi: [i32; 3]) -> bool {
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
            if cs[a] < lo[a] {
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
            let fixed = if outside[a] < 0 { lo[a] } else { hi[a] };
            for u in lo[b]..=hi[b] {
                for v in lo[c]..=hi[c] {
                    let mut p = [0i32; 3];
                    p[a] = fixed;
                    p[b] = u;
                    p[c] = v;
                    let slot =
                        self.grid.slot(p[0] as usize, p[1] as usize, p[2] as usize) as u32;
                    // ponytail: `entry = ANY` waives the crossing mask for every exit of a
                    // seeded section, because a walk arriving from outside the grid has no entry
                    // face to name. The ceiling is that the grid's shell on the camera's side is
                    // never culled and can expand one step further than a real entry face would
                    // allow. The upgrade is to seed each boundary section through the face the
                    // camera actually lies beyond.
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
        // [`NEVER`] is every bit set, which is why the first arrival needs no case of its own:
        // intersecting with it leaves exactly what that arrival brought.
        let seen = &mut self.spent[slot as usize * ENTRIES + entry as usize];
        let merged = *seen & dirs as u8;
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

    /// The sections the walk may step onto, in its own grid's coordinates: whichever is narrower
    /// of the grid and the loaded world.
    fn bounds(&self) -> ([i32; 3], [i32; 3]) {
        let extent = self.grid.extent();
        let mut lo = [0i32; 3];
        let mut hi = [0i32; 3];
        for axis in 0..3 {
            let offset = self.world_min[axis] - self.min_section[axis];
            lo[axis] = offset.max(0);
            hi[axis] = (offset + self.world_extent[axis] as i32).min(extent[axis] as i32) - 1;
        }
        (lo, hi)
    }

    /// Re-opens that run, which the walk's own clearing has just closed.
    fn open_the_tail(&mut self) {
        let tail = self.always_visible() as usize / 32;
        self.bits[tail..].fill(u32::MAX);
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
    let started = std::time::Instant::now();
    cave.run(transform.translation(), frustum);
    let slot = cave.walks % CaveCull::TIMED;
    cave.took[slot] = started.elapsed().as_micros() as u32;
    cave.walks += 1;
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
    use crate::pack::{RENDER_REGION_X, RENDER_REGION_Y};
    use bevy::camera::CameraProjection;

    /// A shallow world, so a fixture stays small while still spanning several render regions. Its
    /// height is a whole render region, because the walk may step anywhere in its own grid.
    const SECTIONS: [usize; 3] = [REGION_CHUNKS, RENDER_REGION_Y, REGION_CHUNKS];

    fn grid() -> RegionGrid {
        RegionGrid::covering(SECTIONS)
    }

    fn slot(sx: usize, sy: usize, sz: usize) -> usize {
        grid().slot(sx, sy, sz)
    }

    fn walk(conn: Vec<u64>) -> CaveCull {
        at(conn, [0; 3])
    }

    fn at(conn: Vec<u64>, corner: [i32; 3]) -> CaveCull {
        let mut cave = CaveCull::new(grid(), corner, SECTIONS);
        cave.conn.copy_from_slice(&conn);
        cave
    }

    /// A wall of sealed sections spanning the grid, which is what a test needs to have something
    /// the walk must refuse to see through.
    fn wall_at(sx: usize) -> Vec<u64> {
        let mut conn = open_conn();
        for sy in 0..SECTIONS[1] {
            for sz in 0..SECTIONS[2] {
                conn[slot(sx, sy, sz)] = 0;
            }
        }
        conn
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
        let conn = wall_at(20);
        let mut cave = walk(conn);
        let eye = Vec3::new(900.0, 32.0, 256.0);
        cave.run(eye, &wide(eye, Vec3::new(0.0, 32.0, 256.0)));
        assert!(visible(&cave, 25, 2, 16), "section in front of the wall");
        assert!(!visible(&cave, 10, 2, 16), "section behind the wall");
    }

    /// The walk covers a box around the camera, not the whole loaded world, so geometry can sit
    /// outside it. Those draws point past the grid at a run of slots that is always set: culling
    /// them with a bit nobody wrote would make the far half of the view disappear.
    #[test]
    fn geometry_outside_the_walks_grid_is_drawn_rather_than_culled() {
        let conn = wall_at(20);
        let mut cave = walk(conn);
        let eye = Vec3::new(900.0, 32.0, 256.0);
        cave.run(eye, &wide(eye, Vec3::new(0.0, 32.0, 256.0)));
        assert!(!visible(&cave, 10, 2, 16), "the walk really did cull something");

        let tail = cave.always_visible() as usize;
        for slot in tail..tail + SECTIONS_PER_RENDER_REGION {
            assert!(
                cave.bits[slot >> 5] >> (slot & 31) & 1 != 0,
                "slot {slot} past the grid has to stay set"
            );
        }
    }

    /// Sliding the grid renumbers every slot in it, so what it held before means nothing at the
    /// new corner. Keeping the old masks would cull against rock that is somewhere else now.
    #[test]
    fn sliding_the_grid_forgets_what_it_covered() {
        let mut conn = open_conn();
        conn[slot(5, 2, 5)] = 0;
        let mut cave = walk(conn);
        assert_eq!(cave.conn[slot(5, 2, 5)], 0);

        cave.retarget([RENDER_REGION_X as i32, 0, 0]);
        assert_eq!(cave.min_section(), [RENDER_REGION_X as i32, 0, 0]);
        assert!(
            cave.conn.iter().all(|mask| *mask == CONNECT_ALL),
            "a grid that has slid holds nothing until the loader lays it back in"
        );
    }

    /// Region coordinates run either side of zero, so the window a walk covers can start at a
    /// negative section. Losing that offset puts the camera outside the window instead of inside
    /// it, and the walk then seeds the far boundary and reports the wall's other side.
    #[test]
    fn a_window_below_the_origin_culls_from_where_the_camera_really_is() {
        let corner = [-(REGION_CHUNKS as i32), 0, -(REGION_CHUNKS as i32)];
        let conn = wall_at(20);
        let mut cave = at(conn, corner);
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

    /// Two routes that reach one section through the *same* face, having spent different
    /// directions on the way. Both come from the boundary plane, so both start with west spent;
    /// one then turns south to reach the row, the other north.
    ///
    /// Corridors: a row at each of `z = 12` and `z = 20` running west from the plane to `x = 17`,
    /// and a column at `x = 17` joining them, so both meet at `(17, 2, 16)` and step west into
    /// `(16, 2, 16)` through its east face.
    fn one_face_two_ways() -> CaveCull {
        let mut conn = empty_conn();
        for (row, turn) in [(12usize, pair(5, 3)), (20, pair(5, 2))] {
            for x in 18..SECTIONS[0] {
                conn[slot(x, 2, row)] = pair(5, 4) | pair(4, 5);
            }
            conn[slot(17, 2, row)] = turn;
        }
        for z in 13..16 {
            conn[slot(17, 2, z)] = pair(2, 3);
        }
        for z in 17..20 {
            conn[slot(17, 2, z)] = pair(3, 2);
        }
        conn[slot(17, 2, 16)] = pair(2, 4) | pair(3, 4);
        conn[slot(16, 2, 16)] = pair(5, 2) | pair(5, 3);
        walk(conn)
    }

    /// Arrivals through one face merge by keeping only what both spent. The southbound route
    /// reaches this section having spent south, which forbids it from turning north again; the
    /// northbound route has spent north and cannot turn south. Only by dropping to what the two
    /// have in common does the section open both ways, and a route that really does see through it
    /// either way survives.
    #[test]
    fn a_second_arrival_through_one_face_keeps_only_what_both_spent() {
        let mut cave = one_face_two_ways();
        let eye = Vec3::new(900.0, 40.0, 264.0);
        cave.run(eye, &wide(eye, Vec3::new(0.0, 40.0, 264.0)));
        assert!(visible(&cave, 16, 2, 16), "the section both routes reach");
        assert!(visible(&cave, 16, 2, 15), "north of it, which only the southbound route forbids");
        assert!(visible(&cave, 16, 2, 17), "south of it, which only the northbound route forbids");
    }

    /// A camera diagonally outside the grid seeds two boundary planes, and travelling to each of
    /// them spends that axis. Being outside on a second axis has to forbid stepping back along it
    /// just as the first does, or a sight line could run away from the camera.
    #[test]
    fn a_camera_outside_on_two_axes_spends_both_of_them() {
        let mut conn = empty_conn();
        for x in 18..SECTIONS[0] {
            conn[slot(x, 2, 16)] = pair(5, 4) | pair(4, 5);
        }
        // The one section that offers a way south as well as a way west.
        conn[slot(17, 2, 16)] = pair(5, 4) | pair(5, 3);
        let mut cave = walk(conn);
        let eye = Vec3::new(900.0, 40.0, 900.0);
        cave.run(eye, &wide(eye, Vec3::new(0.0, 40.0, 0.0)));
        assert!(visible(&cave, 16, 2, 16), "west of the turn, which the walk may still reach");
        assert!(
            !visible(&cave, 17, 2, 17),
            "south of the turn: getting to the boundary already spent north"
        );
    }

    /// What the streamer really does: slide the grid, write each region's masks back into it, then
    /// walk. The walk also has to stop at the loaded world rather than at its own grid, which after
    /// a slide are no longer the same box.
    #[test]
    fn a_walk_after_a_slide_culls_against_the_masks_written_back() {
        let mut cave = walk(open_conn());
        cave.retarget([RENDER_REGION_X as i32, 0, 0]);
        for sy in 0..SECTIONS[1] {
            for sz in 0..SECTIONS[2] {
                let (region, local) = grid().split(10, sy, sz);
                cave.set_region(region * SECTIONS_PER_RENDER_REGION, &[(local, 0)]);
            }
        }
        let eye = Vec3::new(900.0, 40.0, 264.0);
        cave.run(eye, &wide(eye, Vec3::new(0.0, 40.0, 264.0)));
        assert!(visible(&cave, 12, 2, 16), "section in front of the wall that was written back");
        assert!(!visible(&cave, 5, 2, 16), "section behind it");
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
