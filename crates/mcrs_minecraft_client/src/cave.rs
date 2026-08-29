use bevy::camera::primitives::{Aabb, Frustum};
use bevy::prelude::*;

use crate::anvil::SECTION_SIZE;
use crate::mesh::CONNECT_ALL;
use crate::pack::MAX_SECTIONS;

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

/// The walk box in sections, and how far it slides at a time. It is centred on the camera and
/// bounded by sight, not by the world: a section outside it is drawn rather than tested.
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

#[derive(Resource)]
pub struct CaveCull {
    pub enabled: bool,
    pub bits: Box<[u32]>,
    min_section: [i32; 3],
    reached: Box<[u32]>,
    inside: Box<[u32]>,
    spent: Vec<u8>,
    conn: Vec<u64>,
    slot: Vec<u32>,
    queue: Vec<u32>,
    took: Box<[u32; CaveCull::TIMED]>,
    walks: usize,
}

impl CaveCull {
    pub fn new() -> Self {
        Self {
            enabled: !std::env::var("ANVIL_CAVE").is_ok_and(|on| on == "0"),
            bits: vec![u32::MAX; MAX_SECTIONS / 32].into_boxed_slice(),
            min_section: [0; 3],
            reached: vec![0; WALK_CELLS / 32].into_boxed_slice(),
            inside: vec![0; WALK_CELLS / 32].into_boxed_slice(),
            spent: vec![NEVER; WALK_CELLS * ENTRIES],
            conn: vec![CONNECT_ALL; WALK_CELLS],
            slot: vec![NO_SLOT; WALK_CELLS],
            queue: Vec::with_capacity(WALK_CELLS),
            took: Box::new([0; Self::TIMED]),
            walks: 0,
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
        self.reached.iter().map(|word| word.count_ones()).sum()
    }

    /// Slides the walk box onto the camera, in whole steps. A box that moved holds nothing until
    /// the loader lays its resident sections back in.
    pub fn follow(&mut self, camera: Vec3) -> bool {
        let corner = std::array::from_fn(|axis| {
            let here = (camera[axis] / SECTION_SIZE as f32).floor() as i32;
            here.div_euclid(WALK_STEP) * WALK_STEP - WALK[axis] as i32 / 2
        });
        if corner == self.min_section {
            return false;
        }
        self.min_section = corner;
        self.conn.fill(CONNECT_ALL);
        self.slot.fill(NO_SLOT);
        true
    }

    pub fn set_section(&mut self, section: [i32; 3], slot: u32, mask: u64) {
        let Some(cell) = self.cell(section) else {
            return;
        };
        self.conn[cell] = mask;
        self.slot[cell] = slot;
    }

    pub fn forget(&mut self, section: [i32; 3]) {
        let Some(cell) = self.cell(section) else {
            return;
        };
        self.conn[cell] = CONNECT_ALL;
        self.slot[cell] = NO_SLOT;
    }

    fn cell(&self, section: [i32; 3]) -> Option<usize> {
        let mut local = [0usize; 3];
        for axis in 0..3 {
            let at = section[axis] - self.min_section[axis];
            if at < 0 || at as usize >= WALK[axis] {
                return None;
            }
            local[axis] = at as usize;
        }
        Some((local[1] * WALK[2] + local[2]) * WALK[0] + local[0])
    }

    fn section_at(&self, cell: usize) -> [i32; 3] {
        let x = cell % WALK[0];
        let rest = cell / WALK[0];
        [
            x as i32 + self.min_section[0],
            (rest / WALK[2]) as i32 + self.min_section[1],
            (rest % WALK[2]) as i32 + self.min_section[2],
        ]
    }

    fn run(&mut self, camera: Vec3, frustum: &Frustum) {
        self.bits.fill(u32::MAX);
        let here = std::array::from_fn(|axis| {
            (camera[axis] / SECTION_SIZE as f32).floor() as i32
        });
        let Some(start) = self.cell(here).filter(|cell| self.conn[*cell] != 0) else {
            return;
        };
        self.spent.fill(NEVER);
        self.reached.fill(0);
        self.inside.fill(0);
        self.queue.clear();
        self.push(start as u32, ENTRY_ANY, 0, frustum);

        let mut head = 0;
        while head < self.queue.len() {
            let node = self.queue[head];
            head += 1;
            let cell = node & ((1 << QUEUE_CELL_BITS) - 1);
            let entry = (node >> QUEUE_ENTRY_SHIFT) & 7;
            let dirs = (node >> QUEUE_DIRS_SHIFT) & 0x3f;
            let here = self.section_at(cell as usize);
            let mask = self.conn[cell as usize];

            for exit in 0..6u32 {
                if dirs & (1 << (exit ^ 1)) != 0 {
                    continue;
                }
                if entry != ENTRY_ANY && mask >> (entry * 6 + exit) & 1 == 0 {
                    continue;
                }
                let step = NEIGHBOUR[exit as usize];
                let next = [here[0] + step[0], here[1] + step[1], here[2] + step[2]];
                let Some(neighbour) = self.cell(next) else {
                    continue;
                };
                self.push(neighbour as u32, exit ^ 1, dirs | 1 << exit, frustum);
            }
        }
        self.project();
    }

    /// The shader indexes visibility by table slot, so what the walk did not reach loses its bit.
    /// A section the box does not cover has no cell here and keeps the bit it was filled with.
    fn project(&mut self) {
        for cell in 0..WALK_CELLS {
            let slot = self.slot[cell];
            if slot == NO_SLOT || self.reached[cell >> 5] >> (cell & 31) & 1 != 0 {
                continue;
            }
            self.bits[(slot >> 5) as usize] &= !(1 << (slot & 31));
        }
    }

    fn push(&mut self, cell: u32, entry: u32, dirs: u32, frustum: &Frustum) {
        let seen = &mut self.spent[cell as usize * ENTRIES + entry as usize];
        let merged = *seen & dirs as u8;
        if merged == *seen {
            return;
        }
        *seen = merged;

        let (word, bit) = ((cell >> 5) as usize, 1u32 << (cell & 31));
        if self.reached[word] & bit == 0 {
            self.reached[word] |= bit;
            if frustum.intersects_obb_identity(&self.aabb(cell)) {
                self.inside[word] |= bit;
            }
        }
        if self.inside[word] & bit == 0 {
            return;
        }
        self.queue
            .push(cell | entry << QUEUE_ENTRY_SHIFT | (merged as u32) << QUEUE_DIRS_SHIFT);
    }

    fn aabb(&self, cell: u32) -> Aabb {
        let section = self.section_at(cell as usize);
        let size = SECTION_SIZE as f32;
        let min = Vec3::new(
            section[0] as f32 * size,
            section[1] as f32 * size,
            section[2] as f32 * size,
        );
        Aabb::from_min_max(min, min + size)
    }
}

impl Default for CaveCull {
    fn default() -> Self {
        Self::new()
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

pub fn toggle(keys: Res<ButtonInput<KeyCode>>, mut cave: ResMut<CaveCull>) {
    if keys.just_pressed(KeyCode::KeyC) {
        cave.enabled = !cave.enabled;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::camera::CameraProjection;
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
            let mut cave = CaveCull::new();
            cave.follow(eye);
            cave.conn.fill(0);
            Self {
                cave,
                slots: HashMap::new(),
            }
        }

        fn open(&mut self, section: [i32; 3], mask: u64) {
            let next = self.slots.len() as u32;
            let slot = *self.slots.entry(section).or_insert(next);
            self.cave.set_section(section, slot, mask);
        }

        fn run(&mut self, eye: Vec3, at: Vec3) {
            self.cave.run(eye, &wide(eye, at));
        }

        fn visible(&self, section: [i32; 3]) -> bool {
            let slot = self.slots[&section];
            self.cave.bits[(slot >> 5) as usize] >> (slot & 31) & 1 != 0
        }
    }

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
        slab.run(middle([35, 2, 16]), middle([0, 2, 16]));
        assert!(slab.visible([25, 2, 16]), "section in front of the wall");
        assert!(!slab.visible([10, 2, 16]), "section behind the wall");
    }

    #[test]
    fn a_storey_below_the_origin_culls_from_where_the_camera_really_is() {
        let mut slab = walled(-40, 0, -20, [-5, -2, -16]);
        slab.run(middle([-5, -2, -16]), middle([-40, -2, -16]));
        assert!(slab.visible([-15, -2, -16]), "section in front of the wall");
        assert!(!slab.visible([-30, -2, -16]), "section behind the wall");
    }

    #[test]
    fn geometry_outside_the_walk_box_is_drawn_rather_than_culled() {
        let mut slab = walled(0, 40, 20, [35, 2, 16]);
        slab.open([100_000, 0, 0], CONNECT_ALL);
        slab.run(middle([35, 2, 16]), middle([0, 2, 16]));
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
            slab.cave.conn.iter().all(|mask| *mask == CONNECT_ALL),
            "a box that has slid holds nothing until the loader lays it back in"
        );
        assert!(slab.cave.slot.iter().all(|slot| *slot == NO_SLOT));
    }

    #[test]
    fn a_walk_after_a_slide_culls_against_the_masks_written_back() {
        let eye = [35, 2, 16];
        let mut slab = Slab::around(middle(eye));
        slab.cave.follow(middle(eye) + Vec3::X * (WALK[0] * SECTION_SIZE) as f32);
        slab.cave.follow(middle(eye));
        slab.cave.conn.fill(0);
        for x in 0..40 {
            for z in 8..24 {
                slab.open([x, 2, z], if x == 20 { 0 } else { CONNECT_ALL });
            }
        }
        slab.run(middle(eye), middle([0, 2, 16]));
        assert!(slab.visible([25, 2, 16]), "in front of the wall laid back in");
        assert!(!slab.visible([10, 2, 16]), "behind it");
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
        slab.run(middle(eye), middle([16, 2, 20]));
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

        slab.run(middle(eye), middle([4, 2, 4]));
        assert!(slab.visible([4, 3, 4]), "the way up, entered from the north");
        assert!(slab.visible([4, 1, 4]), "the way down, entered from the west");
    }

    #[test]
    fn a_camera_sealed_in_rock_gives_up_instead_of_culling_everything() {
        let mut slab = walled(0, 40, 20, [35, 2, 16]);
        slab.open([35, 2, 16], 0);
        slab.run(middle([35, 2, 16]), middle([0, 2, 16]));
        assert!(
            slab.slots.keys().all(|section| slab.visible(*section)),
            "a walk that cannot start must not cull anything"
        );
    }
}
