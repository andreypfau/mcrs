use crate::anvil::SECTION_SIZE;

use super::face_normal;
use super::scratch::{BORDER_VOLUME, border_index};

pub const CONNECT_ALL: u64 = (1 << 36) - 1;

pub(super) fn connectivity(occludes: &mut [bool; BORDER_VOLUME]) -> u64 {
    const N: i32 = SECTION_SIZE as i32;
    let mut mask = 0u64;
    let mut stack: Vec<[i32; 3]> = Vec::new();

    for sy in 0..N {
        for sz in 0..N {
            for sx in 0..N {
                if occludes[border_index(sx, sy, sz)] {
                    continue;
                }
                occludes[border_index(sx, sy, sz)] = true;
                stack.push([sx, sy, sz]);

                let mut touched = 0u8;
                while let Some([x, y, z]) = stack.pop() {
                    touched |= (y == 0) as u8
                        | ((y == N - 1) as u8) << 1
                        | ((z == 0) as u8) << 2
                        | ((z == N - 1) as u8) << 3
                        | ((x == 0) as u8) << 4
                        | ((x == N - 1) as u8) << 5;
                    for face in 0..6usize {
                        let n = face_normal(face);
                        let (nx, ny, nz) = (x + n[0], y + n[1], z + n[2]);
                        if nx < 0 || ny < 0 || nz < 0 || nx >= N || ny >= N || nz >= N {
                            continue;
                        }
                        let index = border_index(nx, ny, nz);
                        if occludes[index] {
                            continue;
                        }
                        occludes[index] = true;
                        stack.push([nx, ny, nz]);
                    }
                }

                for entry in 0..6 {
                    if touched >> entry & 1 == 1 {
                        mask |= (touched as u64) << (entry * 6);
                    }
                }
            }
        }
    }
    mask
}

#[cfg(test)]
mod tests {
    use super::{BORDER_VOLUME, border_index, connectivity};

    fn pair(entry: usize, exit: usize) -> u64 {
        1 << (entry * 6 + exit)
    }

    fn solid_section() -> Box<[bool; BORDER_VOLUME]> {
        Box::new([true; BORDER_VOLUME])
    }

    #[test]
    fn a_vertical_shaft_connects_only_down_and_up() {
        let mut occludes = solid_section();
        for y in 0..16 {
            occludes[border_index(8, y, 8)] = false;
        }
        assert_eq!(
            connectivity(&mut occludes),
            pair(0, 0) | pair(0, 1) | pair(1, 0) | pair(1, 1)
        );
    }

    #[test]
    fn two_disjoint_shafts_do_not_join() {
        let mut occludes = solid_section();
        for y in 0..16 {
            occludes[border_index(2, y, 2)] = false;
        }
        for x in 0..16 {
            occludes[border_index(x, 12, 12)] = false;
        }
        let mask = connectivity(&mut occludes);
        assert_eq!(mask & (pair(0, 4) | pair(4, 0)), 0, "shafts must not merge");
        assert_eq!(
            mask,
            pair(0, 0)
                | pair(0, 1)
                | pair(1, 0)
                | pair(1, 1)
                | pair(4, 4)
                | pair(4, 5)
                | pair(5, 4)
                | pair(5, 5)
        );
    }
}
