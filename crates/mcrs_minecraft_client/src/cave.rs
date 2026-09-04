use bevy::platform::time::Instant;
use bevy::prelude::*;
use bevy::tasks::{AsyncComputeTaskPool, Task, futures::check_ready};

use crate::mesh::{Connectivity, OPEN, SEALED, along};
use mcrs_minecraft_network::columns::SECTION_SIZE;

const NEIGHBOUR: [[i32; 3]; 6] = [
    crate::mesh::face_normal(0),
    crate::mesh::face_normal(1),
    crate::mesh::face_normal(2),
    crate::mesh::face_normal(3),
    crate::mesh::face_normal(4),
    crate::mesh::face_normal(5),
];

const ENTRY_ANY: u32 = 6;

const ENTRIES: usize = 7;

const NEVER: u8 = u8::MAX;

const ALL_EXITS: u32 = 0x3f;

/// The steps that would take the walk back along an axis it has already crossed. It never takes
/// one, which is what lets a section be read through the flood for the octant being travelled
/// instead of the unrestricted one.
const fn turning_back(dirs: u32) -> u32 {
    (dirs & 0b010101) << 1 | (dirs & 0b101010) >> 1
}

/// The addressing box in sections, and how far it slides at a time. It is centred on the camera:
/// a section outside it has no cell and is drawn rather than tested. What the walk costs is set by
/// the span of sections laid into the box, not by this.
pub const WALK: [usize; 3] = [96, 32, 96];

const WALK_STEP: i32 = 16;

const WALK_CELLS: usize = WALK[0] * WALK[1] * WALK[2];

pub const NO_SLOT: u32 = u32::MAX;

const QUEUE_CELL_BITS: u32 = 20;
const QUEUE_ENTRY_SHIFT: u32 = QUEUE_CELL_BITS;
const QUEUE_DIRS_SHIFT: u32 = QUEUE_ENTRY_SHIFT + 3;

const _: () = assert!(WALK_CELLS <= 1 << QUEUE_CELL_BITS);

const _: () = assert!(QUEUE_DIRS_SHIFT + 6 <= 32);

const _: () = {
    let mut face = 0;
    while face < 6 {
        let there = crate::mesh::face_normal(face);
        let back = crate::mesh::face_normal(face ^ 1);
        assert!(there[0] == -back[0] && there[1] == -back[1] && there[2] == -back[2]);
        face += 1;
    }
    // Half a box plus the widest step still has to land inside the box, or the camera could
    // stand outside its own walk.
    let mut axis = 0;
    while axis < 3 {
        assert!(WALK[axis] / 2 + WALK_STEP as usize <= WALK[axis]);
        axis += 1;
    }
};

/// Clears cells `from..=to` of a bitset without touching the cells sharing their end words.
fn clear_run(bits: &mut [u32], from: usize, to: usize) {
    let (first, last) = (from >> 5, to >> 5);
    let (head, tail) = (u32::MAX << (from & 31), u32::MAX >> (31 - (to & 31)));
    if first == last {
        bits[first] &= !(head & tail);
        return;
    }
    bits[first] &= !head;
    bits[first + 1..last].fill(0);
    bits[last] &= !tail;
}

fn cell_of(local: [usize; 3]) -> usize {
    (local[1] * WALK[2] + local[2]) * WALK[0] + local[0]
}

fn local_of(cell: usize) -> [usize; 3] {
    let rest = cell / WALK[0];
    [cell % WALK[0], rest / WALK[2], rest % WALK[2]]
}

fn section_of(camera: Vec3) -> [i32; 3] {
    std::array::from_fn(|axis| (camera[axis] / SECTION_SIZE as f32).floor() as i32)
}

fn corner_for(camera: Vec3) -> [i32; 3] {
    let section = section_of(camera);
    std::array::from_fn(|axis| {
        section[axis].div_euclid(WALK_STEP) * WALK_STEP - WALK[axis] as i32 / 2
    })
}

fn local_in(corner: [i32; 3], section: [i32; 3]) -> Option<[usize; 3]> {
    let mut local = [0usize; 3];
    for axis in 0..3 {
        let at = section[axis] - corner[axis];
        if at < 0 || at as usize >= WALK[axis] {
            return None;
        }
        local[axis] = at as usize;
    }
    Some(local)
}

#[derive(Clone, Copy)]
enum Edit {
    Slide([i32; 3]),
    Lay([i32; 3], u32, Connectivity),
    Forget([i32; 3]),
}

/// Which sections a line of sight from the camera can reach through the sections' own
/// connectivity. The truth is the loader's resident set, which reaches the walker as edits; the
/// walk runs on the compute pool whenever the camera's section or the topology has changed, and
/// its bits are adopted here when it lands. Between walks a section is drawn until told
/// otherwise, and the frustum is the GPU cull's business.
#[derive(Resource)]
pub struct CaveCull {
    pub enabled: bool,
    pub bits: Box<[u32]>,
    /// Counts the changes to `bits`, so a reader can skip the frames that made none.
    pub generation: u32,
    corner: [i32; 3],
    edits: Vec<Edit>,
    /// Edits made since the running walk took its tables, so a slot they laid stays drawn
    /// whatever that walk concluded about the section that held the slot before.
    laid_meanwhile: Vec<u32>,
    walked_from: Option<[i32; 3]>,
    changed: bool,
    idle: Option<Walker>,
    walking: Option<Task<Walker>>,
    took: Box<[u32; CaveCull::TIMED]>,
    walks: usize,
    reached: u32,
}

struct Walker {
    min_section: [i32; 3],
    laid: Option<[[usize; 3]; 2]>,
    walked: Option<[[i32; 3]; 2]>,
    reached: Box<[u32]>,
    spent: Vec<u8>,
    conn: Vec<Connectivity>,
    slot: Vec<u32>,
    queue: Vec<u32>,
    bits: Box<[u32]>,
    took_us: u32,
}

impl CaveCull {
    pub fn new(slots: usize) -> Self {
        Self {
            enabled: !std::env::var("MCRS_CAVE").is_ok_and(|on| on == "0"),
            bits: vec![u32::MAX; slots.div_ceil(32)].into_boxed_slice(),
            generation: 0,
            corner: [0; 3],
            edits: Vec::new(),
            laid_meanwhile: Vec::new(),
            walked_from: None,
            changed: false,
            idle: Some(Walker::new(slots)),
            walking: None,
            took: Box::new([0; Self::TIMED]),
            walks: 0,
            reached: 0,
        }
    }

    const TIMED: usize = 256;

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

    pub fn reached(&self) -> u32 {
        self.reached
    }

    /// Slides the walk box onto the camera, in whole steps. A box that moved holds nothing until
    /// the loader lays its resident sections back in.
    pub fn follow(&mut self, camera: Vec3) -> bool {
        let corner = corner_for(camera);
        if corner == self.corner {
            return false;
        }
        self.corner = corner;
        self.edit(Edit::Slide(corner));
        true
    }

    pub fn set_section(&mut self, section: [i32; 3], slot: u32, mask: Connectivity) {
        if local_in(self.corner, section).is_none() {
            return;
        }
        self.edit(Edit::Lay(section, slot, mask));
        if slot != NO_SLOT {
            self.laid_meanwhile.push(slot);
            self.show(slot);
        }
    }

    pub fn forget(&mut self, section: [i32; 3]) {
        if local_in(self.corner, section).is_none() {
            return;
        }
        self.edit(Edit::Forget(section));
    }

    fn edit(&mut self, edit: Edit) {
        self.edits.push(edit);
        self.changed = true;
    }

    fn show(&mut self, slot: u32) {
        let word = &mut self.bits[(slot >> 5) as usize];
        let bit = 1 << (slot & 31);
        if *word & bit == 0 {
            *word |= bit;
            self.generation = self.generation.wrapping_add(1);
        }
    }

    fn adopt(&mut self, walker: Walker) {
        let slot = self.walks % Self::TIMED;
        self.took[slot] = walker.took_us;
        self.walks += 1;
        self.reached = walker.reached();
        if self.bits[..] != walker.bits[..] {
            self.bits.copy_from_slice(&walker.bits);
            self.generation = self.generation.wrapping_add(1);
        }
        for slot in std::mem::take(&mut self.laid_meanwhile) {
            self.show(slot);
        }
        self.idle = Some(walker);
    }

    fn walk_due(&self, camera: Vec3) -> bool {
        self.idle.is_some() && (self.changed || self.walked_from != Some(section_of(camera)))
    }

    fn take_walker(&mut self, camera: Vec3) -> Walker {
        let mut walker = self.idle.take().expect("an idle walker");
        walker.apply(self.edits.drain(..));
        self.laid_meanwhile.clear();
        self.changed = false;
        self.walked_from = Some(section_of(camera));
        walker
    }

    /// The walk, run to completion on the calling thread.
    #[cfg(test)]
    fn walk_now(&mut self, camera: Vec3) {
        let mut walker = self.take_walker(camera);
        walker.run(camera);
        self.adopt(walker);
    }

    /// The walker's tables with every edit so far applied.
    #[cfg(test)]
    fn tables(&mut self) -> &mut Walker {
        let walker = self.idle.as_mut().expect("an idle walker");
        walker.apply(self.edits.drain(..));
        walker
    }
}

impl Walker {
    fn new(slots: usize) -> Self {
        Self {
            min_section: [0; 3],
            laid: None,
            walked: None,
            reached: vec![0; WALK_CELLS / 32].into_boxed_slice(),
            spent: vec![NEVER; WALK_CELLS * ENTRIES],
            conn: vec![OPEN; WALK_CELLS],
            slot: vec![NO_SLOT; WALK_CELLS],
            queue: Vec::with_capacity(WALK_CELLS),
            bits: vec![u32::MAX; slots.div_ceil(32)].into_boxed_slice(),
            took_us: 0,
        }
    }

    fn apply(&mut self, edits: impl Iterator<Item = Edit>) {
        for edit in edits {
            match edit {
                Edit::Slide(corner) => self.slide(corner),
                Edit::Lay(section, slot, mask) => self.lay(section, slot, mask),
                Edit::Forget(section) => {
                    if let Some(cell) = self.cell(section) {
                        self.conn[cell] = OPEN;
                        self.slot[cell] = NO_SLOT;
                    }
                }
            }
        }
    }

    fn slide(&mut self, corner: [i32; 3]) {
        self.min_section = corner;
        self.laid = None;
        self.conn.fill(OPEN);
        self.slot.fill(NO_SLOT);
        // A cell is numbered against the corner, so sliding renumbers every one of them and the
        // last walk's marks can no longer be found by the box they were made in.
        self.walked = None;
        self.spent.fill(NEVER);
        self.reached.fill(0);
    }

    fn lay(&mut self, section: [i32; 3], slot: u32, mask: Connectivity) {
        let Some(local) = self.local(section) else {
            return;
        };
        let cell = cell_of(local);
        self.conn[cell] = mask;
        self.slot[cell] = slot;
        let [lo, hi] = self.laid.get_or_insert([local, local]);
        for axis in 0..3 {
            lo[axis] = lo[axis].min(local[axis]);
            hi[axis] = hi[axis].max(local[axis]);
        }
    }

    fn local(&self, section: [i32; 3]) -> Option<[usize; 3]> {
        local_in(self.min_section, section)
    }

    fn cell(&self, section: [i32; 3]) -> Option<usize> {
        self.local(section).map(cell_of)
    }

    fn reached(&self) -> u32 {
        self.reached.iter().map(|word| word.count_ones()).sum()
    }

    fn run(&mut self, camera: Vec3) {
        let started = Instant::now();
        self.bits.fill(u32::MAX);
        // Only the last walk's box can hold stale marks, so scrubbing it — before any early
        // return — is what leaves the whole of `reached` and `spent` clean again.
        if let Some(walked) = self.walked.take() {
            self.scrub(walked);
        }
        let section = section_of(camera);
        let (Some(eye), Some([laid_lo, laid_hi])) = (self.local(section), self.laid) else {
            self.took_us = started.elapsed().as_micros() as u32;
            return;
        };
        let start = cell_of(eye);
        if self.conn[start] == SEALED {
            self.took_us = started.elapsed().as_micros() as u32;
            return;
        }
        // The walk never turns back along an axis it has already stepped, so its path between two
        // cells stays between them on every axis. Clipping it to the sections laid in — widened to
        // wherever the camera stands — therefore drops no cell it could have reached.
        let lo: [i32; 3] = std::array::from_fn(|axis| laid_lo[axis].min(eye[axis]) as i32);
        let hi: [i32; 3] = std::array::from_fn(|axis| laid_hi[axis].max(eye[axis]) as i32);
        self.walked = Some([lo, hi]);
        self.queue.clear();
        self.push(start as u32, ENTRY_ANY, 0);

        let mut head = 0;
        while head < self.queue.len() {
            let node = self.queue[head];
            head += 1;
            let cell = node & ((1 << QUEUE_CELL_BITS) - 1);
            let entry = (node >> QUEUE_ENTRY_SHIFT) & 7;
            let dirs = (node >> QUEUE_DIRS_SHIFT) & 0x3f;
            let here = local_of(cell as usize).map(|at| at as i32);
            let open = match entry {
                ENTRY_ANY => ALL_EXITS,
                entry => (along(&self.conn[cell as usize], dirs) >> (entry * 6)) as u32 & ALL_EXITS,
            };
            let mut exits = open & !turning_back(dirs);

            while exits != 0 {
                let exit = exits.trailing_zeros();
                exits &= exits - 1;
                let step = NEIGHBOUR[exit as usize];
                let next = [here[0] + step[0], here[1] + step[1], here[2] + step[2]];
                if (0..3).any(|axis| next[axis] < lo[axis] || next[axis] > hi[axis]) {
                    continue;
                }
                let neighbour = cell_of(next.map(|at| at as usize));
                self.push(neighbour as u32, exit ^ 1, dirs | 1 << exit);
            }
        }
        self.project(laid_lo, laid_hi);
        self.took_us = started.elapsed().as_micros() as u32;
    }

    fn scrub(&mut self, [lo, hi]: [[i32; 3]; 2]) {
        for y in lo[1]..=hi[1] {
            for z in lo[2]..=hi[2] {
                let row = (y as usize * WALK[2] + z as usize) * WALK[0];
                let (from, to) = (row + lo[0] as usize, row + hi[0] as usize);
                self.spent[from * ENTRIES..(to + 1) * ENTRIES].fill(NEVER);
                clear_run(&mut self.reached, from, to);
            }
        }
    }

    /// The shader indexes visibility by table slot, so what the walk did not reach loses its bit.
    /// A section the box does not cover has no cell here and keeps the bit it was filled with.
    fn project(&mut self, lo: [usize; 3], hi: [usize; 3]) {
        for y in lo[1]..=hi[1] {
            for z in lo[2]..=hi[2] {
                let row = (y * WALK[2] + z) * WALK[0];
                for cell in row + lo[0]..=row + hi[0] {
                    let slot = self.slot[cell];
                    if slot == NO_SLOT || self.reached[cell >> 5] >> (cell & 31) & 1 != 0 {
                        continue;
                    }
                    self.bits[(slot >> 5) as usize] &= !(1 << (slot & 31));
                }
            }
        }
    }

    fn push(&mut self, cell: u32, entry: u32, dirs: u32) {
        let seen = &mut self.spent[cell as usize * ENTRIES + entry as usize];
        let merged = *seen & dirs as u8;
        if merged == *seen {
            return;
        }
        *seen = merged;
        self.reached[(cell >> 5) as usize] |= 1u32 << (cell & 31);
        self.queue
            .push(cell | entry << QUEUE_ENTRY_SHIFT | (merged as u32) << QUEUE_DIRS_SHIFT);
    }
}

pub fn cave_cull(
    mut cave: ResMut<CaveCull>,
    camera: Single<&GlobalTransform, With<Camera3d>>,
) {
    let cave = cave.bypass_change_detection();
    if !cave.enabled {
        if cave.bits.iter().any(|&word| word != u32::MAX) {
            cave.bits.fill(u32::MAX);
            cave.generation = cave.generation.wrapping_add(1);
        }
        return;
    }
    if let Some(task) = cave.walking.as_mut()
        && let Some(walker) = check_ready(task)
    {
        cave.walking = None;
        cave.adopt(walker);
    }
    let camera = camera.translation();
    if cave.walk_due(camera) {
        let mut walker = cave.take_walker(camera);
        cave.walking = Some(AsyncComputeTaskPool::get().spawn(async move {
            walker.run(camera);
            walker
        }));
    }
}

pub fn toggle(keys: Res<ButtonInput<KeyCode>>, mut cave: ResMut<CaveCull>) {
    if keys.just_pressed(KeyCode::KeyC) {
        cave.enabled = !cave.enabled;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mesh::CONNECT_ALL;
    use std::collections::HashMap;

    /// The walk box is full of open air, so a test builds its topology into solid rock and opens
    /// only the sections it means to talk about. A section gets a table slot the first time it is
    /// opened, whatever its mask, because visibility is asked of slots.
    struct Slab {
        cave: CaveCull,
        slots: HashMap<[i32; 3], u32>,
    }

    impl Slab {
        fn around(eye: Vec3) -> Self {
            let mut slab = Self::in_the_open(eye);
            slab.cave.tables().conn.fill(SEALED);
            slab
        }

        /// Every cell as the loader leaves one it has never reached: open air with no slot.
        fn in_the_open(eye: Vec3) -> Self {
            let mut cave = CaveCull::new(WALK_CELLS);
            cave.follow(eye);
            Self {
                cave,
                slots: HashMap::new(),
            }
        }

        fn open(&mut self, section: [i32; 3], mask: u64) {
            self.open_along(section, [mask; 4]);
        }

        fn open_along(&mut self, section: [i32; 3], mask: Connectivity) {
            let next = self.slots.len() as u32;
            let slot = *self.slots.entry(section).or_insert(next);
            self.cave.set_section(section, slot, mask);
        }

        fn run(&mut self, eye: Vec3) {
            self.cave.walk_now(eye);
        }

        fn visible(&self, section: [i32; 3]) -> bool {
            let slot = self.slots[&section];
            self.cave.bits[(slot >> 5) as usize] >> (slot & 31) & 1 != 0
        }
    }


    fn pair(entry: u32, exit: u32) -> u64 {
        1 << (entry * 6 + exit)
    }

    fn middle(section: [i32; 3]) -> Vec3 {
        Vec3::new(
            section[0] as f32 + 0.5,
            section[1] as f32 + 0.5,
            section[2] as f32 + 0.5,
        ) * SECTION_SIZE as f32
    }

    /// One open storey with a wall across it, running from `from` in x to `to`.
    fn walled(from: i32, to: i32, wall: i32, eye: [i32; 3]) -> Slab {
        let mut slab = Slab::around(middle(eye));
        for x in from..to {
            for z in eye[2] - 8..eye[2] + 8 {
                slab.open([x, eye[1], z], if x == wall { 0 } else { CONNECT_ALL });
            }
        }
        slab
    }

    #[test]
    fn a_solid_wall_hides_what_is_behind_it() {
        let mut slab = walled(0, 40, 20, [35, 2, 16]);
        slab.run(middle([35, 2, 16]));
        assert!(slab.visible([25, 2, 16]), "section in front of the wall");
        assert!(!slab.visible([10, 2, 16]), "section behind the wall");
    }

    #[test]
    fn a_storey_below_the_origin_culls_from_where_the_camera_really_is() {
        let mut slab = walled(-40, 0, -20, [-5, -2, -16]);
        slab.run(middle([-5, -2, -16]));
        assert!(slab.visible([-15, -2, -16]), "section in front of the wall");
        assert!(!slab.visible([-30, -2, -16]), "section behind the wall");
    }

    #[test]
    fn geometry_outside_the_walk_box_is_drawn_rather_than_culled() {
        let mut slab = walled(0, 40, 20, [35, 2, 16]);
        slab.open([100_000, 0, 0], CONNECT_ALL);
        slab.run(middle([35, 2, 16]));
        assert!(
            !slab.visible([10, 2, 16]),
            "the walk really did cull something"
        );
        assert!(
            slab.visible([100_000, 0, 0]),
            "a section the box does not cover has to stay drawn"
        );
    }

    #[test]
    fn sliding_the_box_forgets_what_it_covered() {
        let mut slab = walled(0, 40, 20, [35, 2, 16]);
        let far = middle([35, 2, 16]) + Vec3::X * (WALK[0] * SECTION_SIZE) as f32;
        assert!(slab.cave.follow(far), "a whole box away has to move it");
        assert!(
            slab.cave.tables().conn.iter().all(|mask| *mask == OPEN),
            "a box that has slid holds nothing until the loader lays it back in"
        );
        assert!(slab.cave.tables().slot.iter().all(|slot| *slot == NO_SLOT));
    }

    #[test]
    fn a_walk_after_a_slide_culls_against_the_masks_written_back() {
        let eye = [35, 2, 16];
        let mut slab = Slab::around(middle(eye));
        slab.cave
            .follow(middle(eye) + Vec3::X * (WALK[0] * SECTION_SIZE) as f32);
        slab.cave.follow(middle(eye));
        slab.cave.tables().conn.fill(SEALED);
        for x in 0..40 {
            for z in 8..24 {
                slab.open([x, 2, z], if x == 20 { 0 } else { CONNECT_ALL });
            }
        }
        slab.run(middle(eye));
        assert!(
            slab.visible([25, 2, 16]),
            "in front of the wall laid back in"
        );
        assert!(!slab.visible([10, 2, 16]), "behind it");
    }

    #[test]
    fn a_slide_leaves_no_mark_from_the_walk_before_it() {
        let before = middle([44, 2, 16]);
        let mut slab = Slab::around(before);
        slab.cave.tables().conn.fill(SEALED);
        for x in 40..80 {
            for z in 8..24 {
                slab.open([x, 2, z], CONNECT_ALL);
            }
        }
        slab.run(before);
        assert!(
            slab.cave.tables().spent.iter().any(|seen| *seen != NEVER),
            "the walk has to leave marks for this to be worth asserting"
        );

        assert!(
            slab.cave.follow(middle([60, 2, 16])),
            "the camera crossed a step"
        );
        assert!(
            slab.cave.tables().spent.iter().all(|seen| *seen == NEVER),
            "a cell is numbered against the corner, so a slide renumbers every one of them \
             and no mark may be left behind to be read as another cell's"
        );
    }

    #[test]
    fn a_corner_section_turns_only_the_way_its_mask_allows() {
        let eye = [16, 2, 15];
        let mut slab = Slab::around(middle(eye));
        slab.open(eye, CONNECT_ALL);
        slab.open([16, 2, 16], pair(2, 4) | pair(4, 2));
        slab.open([15, 2, 16], CONNECT_ALL);
        slab.open([17, 2, 16], CONNECT_ALL);
        slab.open([16, 3, 16], CONNECT_ALL);
        slab.run(middle(eye));
        assert!(slab.visible([15, 2, 16]), "the turn to the west");
        assert!(!slab.visible([17, 2, 16]), "east is closed by the mask");
        assert!(!slab.visible([16, 3, 16]), "up is closed by the mask");
    }

    #[test]
    fn a_section_reached_twice_opens_the_exits_of_both_ways_in() {
        let eye = [0, 2, 0];
        let mut slab = Slab::around(middle(eye));
        slab.open(eye, CONNECT_ALL);
        for step in 1..4 {
            slab.open([step, 2, 0], pair(4, 5));
            slab.open([0, 2, step], pair(2, 3));
            slab.open([4, 2, step], pair(2, 3));
            slab.open([step, 2, 4], pair(4, 5));
        }
        slab.open([4, 2, 0], pair(4, 3));
        slab.open([0, 2, 4], pair(2, 5));
        slab.open([4, 2, 4], pair(2, 1) | pair(4, 0));
        slab.open([4, 3, 4], CONNECT_ALL);
        slab.open([4, 1, 4], CONNECT_ALL);

        slab.run(middle(eye));
        assert!(
            slab.visible([4, 3, 4]),
            "the way up, entered from the north"
        );
        assert!(
            slab.visible([4, 1, 4]),
            "the way down, entered from the west"
        );
    }

    #[test]
    fn the_walk_crosses_air_the_loader_never_laid_in_without_wandering_the_box() {
        let eye = [0, 2, 0];
        let mut slab = Slab::in_the_open(middle(eye));
        slab.open([10, 2, 0], CONNECT_ALL);
        slab.run(middle(eye));

        assert!(
            slab.visible([10, 2, 0]),
            "nine sections of empty air must not stop the walk"
        );
        assert!(
            slab.cave.reached() <= 11,
            "the walk must cost what it can reach, not the {WALK_CELLS} cells of the box: {}",
            slab.cave.reached()
        );
    }

    /// By the time the walk turns west out of the corner it has already stepped west, down and
    /// north, so only that octant's flood is asked. A route through the corner that doubles back on
    /// an axis lives in the other floods and is culled.
    #[test]
    fn a_walk_whose_box_moved_reports_only_what_this_walk_reached() {
        let mut slab = Slab::in_the_open(middle([0, 2, 0]));
        slab.open([10, 2, 0], CONNECT_ALL);

        slab.run(middle([0, 2, 0]));
        assert_eq!(
            slab.cave.reached(),
            11,
            "the eleven cells east to the section laid in"
        );

        slab.run(middle([20, 2, 0]));
        assert_eq!(
            slab.cave.reached(),
            11,
            "the eleven cells west of the new eye, and none the first walk left behind"
        );
    }

    #[test]
    fn a_corner_reads_only_the_flood_for_the_octant_it_was_reached_in() {
        let eye = [8, 8, 8];
        let doubling_back = |corner: Connectivity| {
            let mut slab = Slab::around(middle(eye));
            slab.open(eye, CONNECT_ALL);
            slab.open([7, 8, 8], CONNECT_ALL);
            slab.open([7, 7, 8], CONNECT_ALL);
            slab.open_along([7, 7, 7], corner);
            slab.open([6, 7, 7], CONNECT_ALL);
            slab.run(middle(eye));
            slab
        };

        let mut west_north_down = [CONNECT_ALL; 4];
        west_north_down[0] &= !pair(3, 4);
        let tight = doubling_back(west_north_down);
        assert!(
            tight.visible([7, 7, 7]),
            "the corner itself is still reached"
        );
        assert!(
            !tight.visible([6, 7, 7]),
            "west out of the corner is only in the floods this walk cannot be in"
        );

        assert!(
            doubling_back([CONNECT_ALL; 4]).visible([6, 7, 7]),
            "an unrestricted corner still lets the walk through"
        );
    }

    #[test]
    fn a_camera_sealed_in_rock_gives_up_instead_of_culling_everything() {
        let mut slab = walled(0, 40, 20, [35, 2, 16]);
        slab.open([35, 2, 16], 0);
        slab.run(middle([35, 2, 16]));
        assert!(
            slab.slots.keys().all(|section| slab.visible(*section)),
            "a walk that cannot start must not cull anything"
        );
    }
}
