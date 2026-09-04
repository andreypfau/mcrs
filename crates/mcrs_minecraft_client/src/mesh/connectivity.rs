use mcrs_minecraft_network::columns::SECTION_SIZE;

use super::scratch::{BORDER_VOLUME, border_index};

pub const CONNECT_ALL: u64 = (1 << 36) - 1;

pub const DIRECTIONS: usize = 4;

pub type Connectivity = [u64; DIRECTIONS];

pub const OPEN: Connectivity = [CONNECT_ALL; DIRECTIONS];

pub const SEALED: Connectivity = [0; DIRECTIONS];

/// A step set holds one direction per axis, and a set and its opposite see the same face pairs, so
/// the eight octants a walk can travel in need only four floods.
const STEP_SETS: [u32; DIRECTIONS] = [0b010101, 0b010110, 0b011001, 0b100101];

const OCTANTS: [(u32, usize); 8] = [
    (0b010101, 0),
    (0b010110, 1),
    (0b011001, 2),
    (0b011010, 3),
    (0b100101, 3),
    (0b100110, 2),
    (0b101001, 1),
    (0b101010, 0),
];

const N: i32 = SECTION_SIZE as i32;

const VOLUME: usize = (N * N * N) as usize;

/// Separating any two faces of the section means cutting it, and the smallest cut through a
/// 16³ box is one of its 16×16 slices.
const SMALLEST_CUT: usize = (N * N) as usize;

/// Merges the floods a walk that has already stepped `stepped` could be travelling in. An axis it
/// has not stepped yet is free, so both of its octants apply.
pub fn along(connectivity: &Connectivity, stepped: u32) -> u64 {
    let mut mask = 0;
    for (octant, held) in OCTANTS {
        if octant & stepped == stepped {
            mask |= connectivity[held];
        }
    }
    mask
}

pub(super) fn connectivity(occludes: &[bool; BORDER_VOLUME]) -> Connectivity {
    let mut filled = 0;
    for y in 0..N {
        for z in 0..N {
            for x in 0..N {
                filled += occludes[border_index(x, y, z)] as usize;
            }
        }
    }
    if filled == VOLUME {
        return SEALED;
    }
    if filled < SMALLEST_CUT {
        return OPEN;
    }
    STEP_SETS.map(|steps| resolve(occludes, steps))
}

const fn index(x: i32, y: i32, z: i32) -> usize {
    (x | z << 4 | y << 8) as usize
}

fn sweep(step: i32) -> impl Iterator<Item = i32> {
    (0..N).map(move |at| if step < 0 { N - 1 - at } else { at })
}

/// Restricted to one direction per axis the flood is acyclic, so a single sweep in step order
/// carries every entry face a cell can be reached from, and the cells that step out of the section
/// hand that set to the face they leave through.
fn resolve(occludes: &[bool; BORDER_VOLUME], steps: u32) -> u64 {
    let stepping = |low: usize, high: usize| match steps >> low & 1 {
        0 => (1i32, high, low),
        _ => (-1, low, high),
    };
    let axes = [stepping(4, 5), stepping(0, 1), stepping(2, 3)];

    let mut reached = [0u8; VOLUME];
    let mut out = [0u8; 6];
    for y in sweep(axes[1].0) {
        for z in sweep(axes[2].0) {
            for x in sweep(axes[0].0) {
                if occludes[border_index(x, y, z)] {
                    continue;
                }
                let mut entries = 0u8;
                for (axis, (step, _, entry)) in axes.iter().copied().enumerate() {
                    let mut behind = [x, y, z];
                    behind[axis] -= step;
                    entries |= match (0..N).contains(&behind[axis]) {
                        true => reached[index(behind[0], behind[1], behind[2])],
                        false => 1 << entry,
                    };
                }
                reached[index(x, y, z)] = entries;
                for (axis, (step, exit, _)) in axes.iter().copied().enumerate() {
                    let mut ahead = [x, y, z];
                    ahead[axis] += step;
                    if !(0..N).contains(&ahead[axis]) {
                        out[exit] |= entries;
                    }
                }
            }
        }
    }

    let mut mask = 0u64;
    for (exit, entries) in out.into_iter().enumerate() {
        for entry in 0..6 {
            if entries >> entry & 1 != 0 {
                mask |= 1 << (entry * 6 + exit) | 1 << (exit * 6 + entry);
            }
        }
    }
    mask
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mesh::face_normal;

    const NEIGHBOUR: [[i32; 3]; 6] = [
        face_normal(0),
        face_normal(1),
        face_normal(2),
        face_normal(3),
        face_normal(4),
        face_normal(5),
    ];

    fn pair(entry: usize, exit: usize) -> u64 {
        1 << (entry * 6 + exit)
    }

    fn solid_section() -> Box<[bool; BORDER_VOLUME]> {
        Box::new([true; BORDER_VOLUME])
    }

    /// The unrestricted flood the directional one has to stay inside of.
    fn undirected(occludes: &[bool; BORDER_VOLUME]) -> u64 {
        let mut seen = [false; BORDER_VOLUME];
        let mut mask = 0u64;
        let mut stack: Vec<[i32; 3]> = Vec::new();
        for y in 0..N {
            for z in 0..N {
                for x in 0..N {
                    if occludes[border_index(x, y, z)] || seen[border_index(x, y, z)] {
                        continue;
                    }
                    seen[border_index(x, y, z)] = true;
                    stack.push([x, y, z]);
                    let mut touched = 0u64;
                    while let Some(here) = stack.pop() {
                        for axis in 0..3 {
                            touched |= ((here[axis] == 0) as u64) << (axis_face(axis) * 2);
                            touched |= ((here[axis] == N - 1) as u64) << (axis_face(axis) * 2 + 1);
                        }
                        for step in NEIGHBOUR {
                            let next = [here[0] + step[0], here[1] + step[1], here[2] + step[2]];
                            if next.iter().any(|coord| !(0..N).contains(coord)) {
                                continue;
                            }
                            let at = border_index(next[0], next[1], next[2]);
                            if occludes[at] || seen[at] {
                                continue;
                            }
                            seen[at] = true;
                            stack.push(next);
                        }
                    }
                    for entry in 0..6 {
                        if touched >> entry & 1 == 1 {
                            mask |= touched << (entry * 6);
                        }
                    }
                }
            }
        }
        mask
    }

    fn axis_face(axis: usize) -> usize {
        match axis {
            0 => 2,
            1 => 0,
            _ => 1,
        }
    }

    /// A 16³ layout, solid unless `carve` says otherwise.
    fn section(mut carve: impl FnMut(i32, i32, i32) -> bool) -> Box<[bool; BORDER_VOLUME]> {
        let mut occludes = solid_section();
        for y in 0..N {
            for z in 0..N {
                for x in 0..N {
                    occludes[border_index(x, y, z)] = !carve(x, y, z);
                }
            }
        }
        occludes
    }

    #[test]
    fn a_vertical_shaft_connects_only_down_and_up() {
        let occludes = section(|x, _, z| x == 8 && z == 8);
        let through = pair(0, 1) | pair(1, 0);
        assert_eq!(undirected(&occludes), through | pair(0, 0) | pair(1, 1));
        for mask in connectivity(&occludes) {
            assert_eq!(mask & through, through, "the shaft runs right through");
            assert_eq!(mask & !undirected(&occludes), 0);
        }
    }

    #[test]
    fn two_disjoint_shafts_do_not_join() {
        let occludes = section(|x, y, z| (x == 2 && z == 2) || (y == 12 && z == 12));
        let joined = pair(0, 4) | pair(4, 0);
        assert_eq!(undirected(&occludes) & joined, 0);
        for mask in connectivity(&occludes) {
            assert_eq!(mask & joined, 0, "shafts must not merge");
        }
    }

    #[test]
    fn a_solid_section_connects_nothing() {
        assert_eq!(connectivity(&solid_section()), SEALED);
    }

    #[test]
    fn a_sealed_cavity_reaches_no_face() {
        let occludes = section(|x, y, z| [x, y, z].iter().all(|coord| (4..12).contains(coord)));
        assert_eq!(connectivity(&occludes), SEALED);
    }

    #[test]
    fn too_little_rock_to_cut_the_section_connects_everything() {
        let occludes = section(|x, y, _| x != 0 || y != 0);
        assert_eq!(connectivity(&occludes), OPEN);
        assert_eq!(undirected(&occludes), CONNECT_ALL);
    }

    #[test]
    fn a_straight_corridor_joins_the_two_faces_it_pierces() {
        let occludes = section(|_, y, z| y == 8 && z == 8);
        for mask in connectivity(&occludes) {
            assert_ne!(mask & pair(4, 5), 0, "west to east");
            assert_ne!(mask & pair(5, 4), 0, "east to west");
            assert_eq!(mask & pair(0, 1), 0, "nothing runs down to up");
        }
    }

    /// A tunnel in from the east face that has to turn north before it can carry on west.
    fn doubling_back() -> Box<[bool; BORDER_VOLUME]> {
        section(|x, y, z| {
            y == 8 && ((z == 8 && x >= 8) || (x == 8 && (2..=8).contains(&z)) || (z == 2 && x <= 8))
        })
    }

    #[test]
    fn a_detour_north_is_a_way_through_only_for_a_walk_heading_north() {
        let occludes = doubling_back();
        let through = pair(5, 4);
        assert_ne!(
            undirected(&occludes) & through,
            0,
            "the tunnel does join them"
        );

        let masks = connectivity(&occludes);
        assert_ne!(
            masks[0] & through,
            0,
            "west and north is how the tunnel runs"
        );
        assert_eq!(
            masks[2] & through,
            0,
            "west and south cannot take the detour"
        );
    }

    /// The whole point is that a directional flood is tighter, so the one thing it must never do
    /// is find a face pair the unrestricted flood does not. Swept across every density, so both
    /// early-outs are covered as well as the floods between them.
    #[test]
    fn every_flood_stays_inside_the_unrestricted_one() {
        for percent in 0..=100u32 {
            for seed in 0..4u32 {
                let mut state = (seed << 8 | percent).wrapping_mul(0x9e37_79b9) | 1;
                let occludes = section(|_, _, _| {
                    state ^= state << 13;
                    state ^= state >> 17;
                    state ^= state << 5;
                    state % 100 >= percent
                });
                let loose = undirected(&occludes);
                for (held, mask) in connectivity(&occludes).into_iter().enumerate() {
                    assert_eq!(
                        mask & !loose,
                        0,
                        "flood {held} at {percent}% seed {seed} escaped"
                    );
                }
            }
        }
    }
}
