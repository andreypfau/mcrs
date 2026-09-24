use bevy_math::{IVec3, Vec3};
use mcrs_minecraft_core::Direction;

/// Vanilla's packed light coordinates: block light in the low byte and sky light in the third,
/// each in sixteenths of a level, so 240 is level 15.
pub const FULL_BRIGHT: u32 = 0x00F0_00F0;

#[derive(Copy, Clone, Default, PartialEq, Eq, Debug)]
pub struct Neighbour {
    pub full_block: bool,
    pub solid_render: bool,
    pub light_opaque: bool,
}

impl Neighbour {
    fn shade(self) -> f32 {
        if self.full_block { 0.2 } else { 1.0 }
    }
}

#[derive(Copy, Clone, Default, Debug)]
pub struct Sample {
    pub neighbour: Neighbour,
    pub light: u32,
}

/// Per-vertex occlusion and smooth light coordinates, in the quad's own vertex order.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Lit {
    pub shade: [f32; 4],
    pub light: [u32; 4],
}

/// The light a block is drawn with at a position: its own emission lifts the block channel, and
/// an emissive block is drawn at full brightness whatever the level says.
pub fn light_coords(emissive: bool, emission: u8, light: u8) -> u32 {
    if emissive {
        return FULL_BRIGHT;
    }
    let block = u32::from((light >> 4).max(emission));
    let sky = u32::from(light & 0xf);
    block << 4 | sky << 20
}

pub fn block_light(coords: u32) -> u32 {
    coords & 0xff
}

pub fn sky_light(coords: u32) -> u32 {
    coords >> 16 & 0xff
}

/// Per-direction vertex corner order, each entry selecting `from` (0) or `to` (1) per axis.
/// Winding is counter-clockwise seen from outside the block, matching the client's `FaceInfo`.
const FACE_CORNERS: [[[u8; 3]; 4]; 6] = [
    [[0, 0, 1], [0, 0, 0], [1, 0, 0], [1, 0, 1]], // down
    [[0, 1, 0], [0, 1, 1], [1, 1, 1], [1, 1, 0]], // up
    [[1, 1, 0], [1, 0, 0], [0, 0, 0], [0, 1, 0]], // north
    [[0, 1, 1], [0, 0, 1], [1, 0, 1], [1, 1, 1]], // south
    [[0, 1, 0], [0, 0, 0], [0, 0, 1], [0, 1, 1]], // west
    [[1, 1, 1], [1, 0, 1], [1, 0, 0], [1, 1, 0]], // east
];

pub fn corner(dir: Direction, index: usize, from: Vec3, to: Vec3) -> Vec3 {
    let select = FACE_CORNERS[dir as usize][index];
    Vec3::new(
        if select[0] == 0 { from.x } else { to.x },
        if select[1] == 0 { from.y } else { to.y },
        if select[2] == 0 { from.z } else { to.z },
    )
}

const SIDES: [[Direction; 4]; 6] = [
    [
        Direction::West,
        Direction::East,
        Direction::North,
        Direction::South,
    ], // down
    [
        Direction::East,
        Direction::West,
        Direction::North,
        Direction::South,
    ], // up
    [
        Direction::Up,
        Direction::Down,
        Direction::East,
        Direction::West,
    ], // north
    [
        Direction::West,
        Direction::East,
        Direction::Down,
        Direction::Up,
    ], // south
    [
        Direction::Up,
        Direction::Down,
        Direction::North,
        Direction::South,
    ], // west
    [
        Direction::Down,
        Direction::Up,
        Direction::North,
        Direction::South,
    ], // east
];

const REMAP: [[usize; 4]; 6] = [
    [0, 1, 2, 3],
    [2, 3, 0, 1],
    [3, 0, 1, 2],
    [0, 1, 2, 3],
    [3, 0, 1, 2],
    [1, 2, 3, 0],
];

const D: usize = 0;
const U: usize = 1;
const N: usize = 2;
const S: usize = 3;
const W: usize = 4;
const E: usize = 5;
const FD: usize = 6;
const FU: usize = 7;
const FN: usize = 8;
const FS: usize = 9;
const FW: usize = 10;
const FE: usize = 11;

/// Which face bounds each vertex weighs the four occlusion slots by, pairwise.
const WEIGHTS: [[[usize; 8]; 4]; 6] = [
    [
        [FW, S, FW, FS, W, FS, W, S],
        [FW, N, FW, FN, W, FN, W, N],
        [FE, N, FE, FN, E, FN, E, N],
        [FE, S, FE, FS, E, FS, E, S],
    ],
    [
        [E, S, E, FS, FE, FS, FE, S],
        [E, N, E, FN, FE, FN, FE, N],
        [W, N, W, FN, FW, FN, FW, N],
        [W, S, W, FS, FW, FS, FW, S],
    ],
    [
        [U, FW, U, W, FU, W, FU, FW],
        [U, FE, U, E, FU, E, FU, FE],
        [D, FE, D, E, FD, E, FD, FE],
        [D, FW, D, W, FD, W, FD, FW],
    ],
    [
        [U, FW, FU, FW, FU, W, U, W],
        [D, FW, FD, FW, FD, W, D, W],
        [D, FE, FD, FE, FD, E, D, E],
        [U, FE, FU, FE, FU, E, U, E],
    ],
    [
        [U, S, U, FS, FU, FS, FU, S],
        [U, N, U, FN, FU, FN, FU, N],
        [D, N, D, FN, FD, FN, FD, N],
        [D, S, D, FS, FD, FS, FD, S],
    ],
    [
        [FD, S, FD, FS, D, FS, D, S],
        [FD, N, FD, FN, D, FN, D, N],
        [FU, N, FU, FN, U, FN, U, N],
        [FU, S, FU, FS, U, FS, U, S],
    ],
];

const EPS: f32 = 1.0e-4;
const NEAR_ONE: f32 = 0.9999;

fn bounds(positions: &[Vec3; 4]) -> (Vec3, Vec3) {
    let mut min = Vec3::splat(32.0);
    let mut max = Vec3::splat(-32.0);
    for p in positions {
        min = min.min(*p);
        max = max.max(*p);
    }
    (min, max)
}

/// Whether a face lies on its block's boundary, so its light and occlusion come from the block in
/// front of it rather than from the block itself.
pub fn face_cubic(positions: &[Vec3; 4], dir: Direction, full_block: bool) -> bool {
    let (min, max) = bounds(positions);
    cubic(min, max, dir, full_block)
}

fn cubic(min: Vec3, max: Vec3, dir: Direction, full_block: bool) -> bool {
    match dir {
        Direction::Down => min.y == max.y && (min.y < EPS || full_block),
        Direction::Up => min.y == max.y && (max.y > NEAR_ONE || full_block),
        Direction::North => min.z == max.z && (min.z < EPS || full_block),
        Direction::South => min.z == max.z && (max.z > NEAR_ONE || full_block),
        Direction::West => min.x == max.x && (min.x < EPS || full_block),
        Direction::East => min.x == max.x && (max.x > NEAR_ONE || full_block),
    }
}

fn face_partial(min: Vec3, max: Vec3, dir: Direction) -> bool {
    match dir {
        Direction::Down | Direction::Up => {
            min.x >= EPS || min.z >= EPS || max.x <= NEAR_ONE || max.z <= NEAR_ONE
        }
        Direction::North | Direction::South => {
            min.x >= EPS || min.y >= EPS || max.x <= NEAR_ONE || max.y <= NEAR_ONE
        }
        Direction::West | Direction::East => {
            min.y >= EPS || min.z >= EPS || max.y <= NEAR_ONE || max.z <= NEAR_ONE
        }
    }
}

fn smooth_blend(mut a: u32, mut b: u32, mut c: u32, center: u32) -> u32 {
    let sky = |coords: u32| coords >> 20 & 15;
    let block = |coords: u32| coords >> 4 & 15;
    if sky(center) > 2 || block(center) > 2 {
        for neighbour in [&mut a, &mut b, &mut c] {
            if *neighbour == 0 {
                *neighbour = center;
            } else if sky(*neighbour) == 0 {
                *neighbour |= center & 0x00FF_0000;
            }
        }
    }
    (a + b + c + center) >> 2 & 0x00FF_00FF
}

fn weighted_blend(coords: [u32; 4], weights: [f32; 4]) -> u32 {
    let channel = |of: fn(u32) -> u32| {
        (of(coords[0]) as f32 * weights[0]
            + of(coords[1]) as f32 * weights[1]
            + of(coords[2]) as f32 * weights[2]
            + of(coords[3]) as f32 * weights[3]) as u32
    };
    channel(block_light) & 0xff | (channel(sky_light) & 0xff) << 16
}

/// Vanilla's smooth lighting of one face: occlusion and blended light at each vertex. `at`
/// answers for the block at an offset from the one the face belongs to, and `own` is the light
/// the block itself is drawn with.
pub fn smooth(
    positions: &[Vec3; 4],
    dir: Direction,
    full_block: bool,
    own: u32,
    at: impl Fn(IVec3) -> Sample,
) -> Lit {
    let (min, max) = bounds(positions);
    let cubic = cubic(min, max, dir, full_block);
    let partial = face_partial(min, max, dir);

    let base = if cubic { dir.normal() } else { IVec3::ZERO };
    let c = SIDES[dir as usize];

    let mut shade = [0.0f32; 4];
    let mut light = [0u32; 4];
    let mut permeable = [false; 4];
    for i in 0..4 {
        let side = at(base + c[i].normal());
        shade[i] = side.neighbour.shade();
        light[i] = side.light;
        permeable[i] = !at(base + c[i].normal() + dir.normal())
            .neighbour
            .light_opaque;
    }
    // The `0` side stands in for every blocked corner, including where symmetry would call for
    // the `1` side. "Fixing" it diverges from the client on every inside corner.
    let diagonal = |a: usize, b: usize| {
        if !permeable[a] && !permeable[b] {
            (shade[0], light[0])
        } else {
            let sample = at(base + c[a].normal() + c[b].normal());
            (sample.neighbour.shade(), sample.light)
        }
    };
    let (shade_02, light_02) = diagonal(0, 2);
    let (shade_03, light_03) = diagonal(0, 3);
    let (shade_12, light_12) = diagonal(1, 2);
    let (shade_13, light_13) = diagonal(1, 3);

    let next = at(dir.normal());
    let center_light = if cubic || !next.neighbour.solid_render {
        next.light
    } else {
        own
    };
    let center_shade = at(base).neighbour.shade();

    let slot = [
        (shade[3] + shade[0] + shade_03 + center_shade) * 0.25,
        (shade[2] + shade[0] + shade_02 + center_shade) * 0.25,
        (shade[2] + shade[1] + shade_12 + center_shade) * 0.25,
        (shade[3] + shade[1] + shade_13 + center_shade) * 0.25,
    ];
    let coords = [
        smooth_blend(light[3], light[0], light_03, center_light),
        smooth_blend(light[2], light[0], light_02, center_light),
        smooth_blend(light[2], light[1], light_12, center_light),
        smooth_blend(light[3], light[1], light_13, center_light),
    ];

    let remap = REMAP[dir as usize];
    let mut lit = Lit {
        shade: [0.0; 4],
        light: [0; 4],
    };
    if partial {
        let shape = [
            min.y,
            max.y,
            min.z,
            max.z,
            min.x,
            max.x,
            1.0 - min.y,
            1.0 - max.y,
            1.0 - min.z,
            1.0 - max.z,
            1.0 - min.x,
            1.0 - max.x,
        ];
        for (vertex, table) in WEIGHTS[dir as usize].iter().enumerate() {
            let weights: [f32; 4] =
                std::array::from_fn(|k| shape[table[2 * k]] * shape[table[2 * k + 1]]);
            let blended = slot[0] * weights[0]
                + slot[1] * weights[1]
                + slot[2] * weights[2]
                + slot[3] * weights[3];
            lit.shade[remap[vertex]] = blended.clamp(0.0, 1.0);
            lit.light[remap[vertex]] = weighted_blend(coords, weights);
        }
    } else {
        for vertex in 0..4 {
            lit.shade[remap[vertex]] = slot[vertex];
            lit.light[remap[vertex]] = coords[vertex];
        }
    }
    lit
}

/// Vanilla's vertex byte: the occlusion quantised to 8 bits, then scaled by the face shade.
pub fn shade_byte(ao: f32, face_shade: f32) -> u8 {
    let ao = (ao * 255.0).floor().clamp(0.0, 255.0) as u8;
    ((ao as f32 * face_shade) as i32).clamp(0, 255) as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    const WEST_SHADE: f32 = 0.6;
    const OPEN_SKY: u32 = 240 << 16;

    fn west_face() -> [Vec3; 4] {
        std::array::from_fn(|i| corner(Direction::West, i, Vec3::ZERO, Vec3::ONE))
    }

    fn solid() -> Neighbour {
        Neighbour {
            full_block: true,
            solid_render: true,
            light_opaque: true,
        }
    }

    fn air(_: IVec3) -> Sample {
        Sample {
            neighbour: Neighbour::default(),
            light: OPEN_SKY,
        }
    }

    /// A full block at the origin on a 5x5 floor of full blocks, plus `extra`, under open sky.
    fn on_floor(extra: &[IVec3]) -> impl Fn(IVec3) -> Sample + use<> {
        let extra = extra.to_vec();
        move |p: IVec3| {
            let blocked = p == IVec3::ZERO
                || extra.contains(&p)
                || (p.y == -1 && p.x.abs() <= 2 && p.z.abs() <= 2);
            if blocked {
                Sample {
                    neighbour: solid(),
                    light: 0,
                }
            } else {
                air(p)
            }
        }
    }

    fn lit(world: impl Fn(IVec3) -> Sample) -> Lit {
        smooth(&west_face(), Direction::West, true, 0, world)
    }

    fn bytes(world: impl Fn(IVec3) -> Sample) -> [u8; 4] {
        lit(world).shade.map(|ao| shade_byte(ao, WEST_SHADE))
    }

    #[test]
    fn open_air_leaves_only_the_face_shade() {
        assert_eq!(bytes(air), [153; 4]);
        assert_eq!(lit(air).light, [OPEN_SKY; 4]);
    }

    #[test]
    fn a_floor_darkens_the_bottom_corners() {
        // v0 and v3 are the top corners, v1 and v2 the bottom ones.
        assert_eq!(bytes(on_floor(&[])), [153, 91, 91, 153]);
    }

    #[test]
    fn a_dark_floor_borrows_the_light_of_the_block_in_front() {
        assert_eq!(lit(on_floor(&[])).light, [OPEN_SKY; 4]);
    }

    #[test]
    fn an_inside_corner_reuses_the_first_side_sample() {
        // Blocks the probe past the west face's `north` corner, firing the `corner_12` shortcut.
        // Vanilla substitutes the *up* sample (1.0) there, not the *down* one (0.2): the corner
        // averages to 0.8 rather than 0.6.
        assert_eq!(bytes(on_floor(&[IVec3::new(-2, 0, -1)]))[1], 122);
    }

    #[test]
    fn a_full_block_that_lets_light_through_does_not_short_circuit_a_corner() {
        let glass = |p: IVec3| {
            let full = p.y == -1 || p == IVec3::new(-2, 0, -1);
            Sample {
                neighbour: Neighbour {
                    full_block: full,
                    solid_render: p.y == -1,
                    light_opaque: p.y == -1,
                },
                light: if p.y == -1 { 0 } else { OPEN_SKY },
            }
        };
        let opaque = on_floor(&[IVec3::new(-2, 0, -1)]);
        assert_ne!(bytes(glass)[1], bytes(opaque)[1]);
    }

    #[test]
    fn a_dim_neighbour_darkens_the_corners_it_touches() {
        let dim = 7 << 20;
        let world = |p: IVec3| Sample {
            neighbour: Neighbour::default(),
            light: if p.y > 0 { dim } else { OPEN_SKY },
        };
        let light = lit(world).light.map(sky_light);
        // v0 and v3 are the top corners.
        assert!(light[0] < 240 && light[3] < 240, "{light:?}");
        assert_eq!([light[1], light[2]], [240, 240]);
    }

    #[test]
    fn a_partial_face_weighs_the_corners_by_its_extent() {
        let slab_top: [Vec3; 4] =
            std::array::from_fn(|i| corner(Direction::Up, i, Vec3::ZERO, Vec3::new(1.0, 0.5, 1.0)));
        let world = |p: IVec3| Sample {
            neighbour: Neighbour::default(),
            light: if p.x < 0 { 7 << 20 } else { OPEN_SKY },
        };
        let got = smooth(&slab_top, Direction::Up, false, OPEN_SKY, world)
            .light
            .map(sky_light);
        assert!(got.iter().any(|&l| l < 240), "{got:?}");
        assert!(got.iter().any(|&l| l == 240), "{got:?}");
    }

    #[test]
    fn full_cube_occlusion_only_ever_lands_on_five_bytes() {
        let mut seen = std::collections::BTreeSet::new();
        for mask in 0u32..1 << 9 {
            let world = move |p: IVec3| {
                let cell = (p.y + 1) * 3 + (p.z + 1);
                let blocked = p.x == -1 && (0..9).contains(&cell) && mask >> cell & 1 == 1;
                Sample {
                    neighbour: if blocked {
                        solid()
                    } else {
                        Neighbour::default()
                    },
                    light: 0,
                }
            };
            for ao in lit(world).shade {
                seen.insert(shade_byte(ao, 1.0));
            }
        }
        assert_eq!(
            seen.into_iter().collect::<Vec<_>>(),
            [51, 102, 153, 204, 255]
        );
    }
}
