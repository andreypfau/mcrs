//! Cave culling: which sections a sight line can actually reach from the camera.
//!
//! Once a frame a breadth-first walk starts at the camera's section and steps into a neighbour only
//! when three tests pass: the section being left can be crossed from the entry face to the exit face
//! (the mask the mesher baked), the step does not reverse an axis already spent, and the neighbour's
//! box lands inside the frustum. Everything the walk never reaches sits behind rock and is thrown
//! away before any vertex work.

use bevy::camera::primitives::{Aabb, Frustum};
use bevy::prelude::*;

/// One bit per section slot, `sx | sy << 5 | sz << 10` — the same index `Group::section & 0x7fff`
/// already carries on the GPU. 32768 bits is 1024 words.
pub const WORDS: usize = 1024;

/// Seeds and the camera's own section fan out through all six faces; there is no real face 7.
const ENTRY_ANY: u32 = 7;

/// `FACE_AXES` order: 0 Down(−Y), 1 Up(+Y), 2 North(−Z), 3 South(+Z), 4 West(−X), 5 East(+X).
const NEIGHBOUR: [[i32; 3]; 6] =
    [[0, -1, 0], [0, 1, 0], [0, 0, -1], [0, 0, 1], [-1, 0, 0], [1, 0, 0]];

/// The negative face of each axis, so that `AXIS_FACE[axis] | positive as u32` is the direction.
const AXIS_FACE: [u32; 3] = [4, 0, 2];

#[derive(Resource)]
pub struct CaveCull {
    pub enabled: bool,
    /// Reached sections, uploaded to the GPU as they are. They double as the visited set: a section
    /// that fails the frustum is marked here and never expanded, and the GPU drops it anyway with
    /// its own frustum test.
    pub bits: Box<[u32; WORDS]>,
    /// [`mesh::RegionMesh::connectivity`](crate::mesh::RegionMesh::connectivity), bit
    /// `entry * 6 + exit`.
    conn: Vec<u64>,
    sections_y: i32,
    min_section_y: i32,
    /// `slot | entry << 15 | dirs << 18`. One push per section, so the queue never wraps and stops
    /// growing after the first frame that fills it.
    queue: Vec<u32>,
}

impl CaveCull {
    pub fn new(conn: Vec<u64>, sections_y: usize, min_section_y: i32) -> Self {
        // ponytail: five bits for sy is already an unspoken invariant of the packed section number
        // the mesher and the cull shader share; a world deeper than 512 blocks breaks that packing
        // long before it breaks the walk. The ceiling is one region file. Several regions at once
        // would need a different packing, not a different walk.
        assert!(sections_y <= 32, "the packed section number gives sy five bits");
        Self {
            enabled: true,
            bits: Box::new([u32::MAX; WORDS]),
            conn,
            sections_y: sections_y as i32,
            min_section_y,
            queue: Vec::with_capacity(32 * 32 * sections_y),
        }
    }

    pub fn reached(&self) -> u32 {
        self.bits.iter().map(|word| word.count_ones()).sum()
    }

    fn run(&mut self, camera: Vec3, frustum: &Frustum) {
        self.bits.fill(0);
        self.queue.clear();
        if !self.seed(camera, frustum) {
            self.bits.fill(u32::MAX);
            return;
        }

        let hi = [31, self.sections_y - 1, 31];
        let mut head = 0;
        while head < self.queue.len() {
            let node = self.queue[head];
            head += 1;
            let slot = node & 0x7fff;
            let entry = (node >> 15) & 7;
            let dirs = (node >> 18) & 0x3f;
            let here = [
                (slot & 31) as i32,
                ((slot >> 5) & 31) as i32,
                ((slot >> 10) & 31) as i32,
            ];
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
                // All three axes, and y against `sections_y` rather than 32: sy = 32 does not run
                // off the array, it quietly rolls into the sz field and reads someone else's
                // section.
                if (0..3).any(|a| next[a] < 0 || next[a] > hi[a]) {
                    continue;
                }
                let neighbour = next[0] as u32 | (next[1] as u32) << 5 | (next[2] as u32) << 10;
                // 3. the frustum, inside `push`, at most once per section.
                self.push(neighbour, exit ^ 1, dirs | 1 << exit, frustum);
            }
        }
    }

    /// `false` asks the caller to give up for this frame: with the camera inside a section no sight
    /// line crosses, the walk would die on its six neighbours and leave an empty screen.
    fn seed(&mut self, camera: Vec3, frustum: &Frustum) -> bool {
        let hi = [31, self.sections_y - 1, 31];
        let cs = [
            camera.x.div_euclid(16.0) as i32,
            camera.y.div_euclid(16.0) as i32 - self.min_section_y,
            camera.z.div_euclid(16.0) as i32,
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
            let slot = cs[0] as u32 | (cs[1] as u32) << 5 | (cs[2] as u32) << 10;
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
                    let slot = p[0] as u32 | (p[1] as u32) << 5 | (p[2] as u32) << 10;
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
        let (word, bit) = ((slot >> 5) as usize, 1u32 << (slot & 31));
        if self.bits[word] & bit != 0 {
            return;
        }
        // Marked before the frustum test so each section is tested at most once per frame. A section
        // the frustum rejects stays in the bitset — `in_frustum` in the shader drops it anyway — but
        // is never expanded, so it cannot carry a sight line around an obstacle.
        self.bits[word] |= bit;
        if !frustum.intersects_obb_identity(&self.aabb(slot)) {
            return;
        }
        self.queue.push(slot | entry << 15 | dirs << 18);
    }

    fn aabb(&self, slot: u32) -> Aabb {
        let min = Vec3::new(
            (slot & 31) as f32 * 16.0,
            ((((slot >> 5) & 31) as i32 + self.min_section_y) * 16) as f32,
            ((slot >> 10) & 31) as f32 * 16.0,
        );
        Aabb::from_min_max(min, min + 16.0)
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
    use crate::mesh::{CONNECT_ALL, SECTION_SLOTS};
    use bevy::camera::CameraProjection;

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

    fn visible(cave: &CaveCull, sx: u32, sy: u32, sz: u32) -> bool {
        let slot = sx | sy << 5 | sz << 10;
        cave.bits[(slot >> 5) as usize] >> (slot & 31) & 1 != 0
    }

    /// The plane seed, the connectivity filter and the no-reversal rule all have to be right at once
    /// for this to pass.
    #[test]
    fn a_solid_wall_hides_what_is_behind_it() {
        let mut conn = vec![CONNECT_ALL; SECTION_SLOTS];
        for sy in 0..4u32 {
            for sz in 0..32u32 {
                conn[(20 | sy << 5 | sz << 10) as usize] = 0;
            }
        }
        let mut cave = CaveCull::new(conn, 4, 0);
        let eye = Vec3::new(900.0, 32.0, 256.0);
        cave.run(eye, &wide(eye, Vec3::new(0.0, 32.0, 256.0)));
        assert!(visible(&cave, 25, 2, 16), "section in front of the wall");
        assert!(!visible(&cave, 10, 2, 16), "section behind the wall");
    }

    /// An asymmetric fixture: exactly one pair of faces is open, North↔West. A walk entering from
    /// the north has to leave to the west and nowhere else, which is what pins down the bit order.
    #[test]
    fn a_corner_section_turns_only_the_way_its_mask_allows() {
        let mut conn = vec![0u64; SECTION_SLOTS];
        conn[(16 | 2 << 5 | 15 << 10) as usize] = CONNECT_ALL;
        conn[(16 | 2 << 5 | 16 << 10) as usize] = 1 << (2 * 6 + 4) | 1 << (4 * 6 + 2);
        let mut cave = CaveCull::new(conn, 4, 0);
        let eye = Vec3::new(264.0, 40.0, 248.0);
        cave.run(eye, &wide(eye, Vec3::new(264.0, 40.0, 400.0)));
        assert!(visible(&cave, 15, 2, 16), "the turn to the west");
        assert!(!visible(&cave, 17, 2, 16), "east is closed by the mask");
        assert!(!visible(&cave, 16, 3, 16), "up is closed by the mask");
    }

    /// A camera inside rock has nothing to start from; culling has to switch itself off for the
    /// frame rather than leave an empty screen.
    #[test]
    fn a_camera_sealed_in_rock_gives_up_instead_of_culling_everything() {
        let mut cave = CaveCull::new(vec![0u64; SECTION_SLOTS], 4, 0);
        let eye = Vec3::new(264.0, 40.0, 264.0);
        cave.run(eye, &wide(eye, Vec3::new(264.0, 40.0, 400.0)));
        assert_eq!(cave.reached(), WORDS as u32 * 32);
    }
}
