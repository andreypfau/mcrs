use crate::anvil::SECTION_SIZE;
use crate::atlas::SpriteRef;
use crate::blocks::{BlockInfo, Fluid, Pass, TintKind};
use crate::pack::{
    FACE_AO, FACE_ARRAY, FACE_BLOCK_LIGHT, FACE_FLUID, FACE_LAYER, FACE_NONE, FACE_SKY_LIGHT,
    FACE_TINT, FLUID_INSET, MODEL_STEPS,
};

use super::model::{self, SHADE_DOWN, SHADE_EAST_WEST, SHADE_NORTH_SOUTH, SHADE_UP, UNGROUPED};
use super::scratch::{
    COLUMNS, Columns, FLUID_KINDS, NOT_FLAT, Scratch, border_index, column_index, section_cell,
};
use super::sweep::{FLUID_KEY, PASS_KEY_BITS, sweep};
use super::{Sink, face_normal};

pub(super) const FLUID_AMOUNT: u8 = 0x0f;
pub(super) const FLUID_LAVA: u8 = 0x10;

const FLUID_FULL: u8 = 9;

pub(super) const COVER_SEE_THROUGH: u8 = 1 << 6;

const IN_SECTION: u32 = ((1 << SECTION_SIZE) - 1) << 1;

pub(super) struct Sloped {
    cell: [i32; 3],
    corners: [f32; 4],
    flow: Option<f32>,
}

#[inline]
pub(super) fn fluid_kind(cell: u8) -> usize {
    (cell & FLUID_LAVA != 0) as usize
}

#[inline]
fn same_fluid(cell: u8, kind: usize) -> bool {
    cell != 0 && fluid_kind(cell) == kind
}

#[inline]
fn drop_steps(ninths: u8) -> u8 {
    (f32::from(FLUID_FULL - ninths) * MODEL_STEPS / f32::from(FLUID_FULL)).round() as u8
}

pub(super) fn surfaces(scratch: &mut Scratch) {
    scratch.sloped.clear();
    if scratch.fluid_kinds == 0 {
        return;
    }
    *scratch.flat_columns = [[[0; COLUMNS]; 3]; FLUID_KINDS];
    scratch.flat_drop.fill(NOT_FLAT);

    for z in 0..SECTION_SIZE as i32 {
        for x in 0..SECTION_SIZE as i32 {
            let column = column_index(1, x, 0, z);
            let mut rows = (scratch.fluid_columns[0][1][column]
                | scratch.fluid_columns[1][1][column])
                & IN_SECTION;
            while rows != 0 {
                let y = rows.trailing_zeros() as i32 - 1;
                rows &= rows - 1;
                let cell = scratch.fluid[border_index(x, y, z)];
                let kind = fluid_kind(cell);
                let above = scratch.fluid[border_index(x, y + 1, z)];
                let own = if same_fluid(above, kind) {
                    FLUID_FULL
                } else {
                    cell & FLUID_AMOUNT
                };
                if own == FLUID_FULL {
                    mark_flat(scratch, [x, y, z], kind, 0);
                    continue;
                }

                let corners = [
                    corner_height(scratch, [x, y, z], kind, own, -1, -1),
                    corner_height(scratch, [x, y, z], kind, own, -1, 1),
                    corner_height(scratch, [x, y, z], kind, own, 1, 1),
                    corner_height(scratch, [x, y, z], kind, own, 1, -1),
                ];
                let flow = flow_angle(scratch, [x, y, z], kind, own);
                let level = exact_ninths(corners[0]).filter(|_| {
                    flow.is_none()
                        && corners.iter().all(|&c| exact_ninths(c) == exact_ninths(corners[0]))
                });
                match level {
                    Some(ninths) => mark_flat(scratch, [x, y, z], kind, drop_steps(ninths)),
                    None => scratch.sloped.push(Sloped {
                        cell: [x, y, z],
                        corners: corners
                            .map(|(num, den)| num as f32 / (den as f32 * f32::from(FLUID_FULL))),
                        flow,
                    }),
                }
            }
        }
    }
}

fn mark_flat(scratch: &mut Scratch, [x, y, z]: [i32; 3], kind: usize, drop: u8) {
    scratch.flat_drop[section_cell(x, y, z)] = drop;
    let along = [x, y, z];
    for axis in 0..3 {
        scratch.flat_columns[kind][axis][column_index(axis, x, y, z)] |= 1 << (along[axis] + 1);
    }
}

#[inline]
fn exact_ninths((num, den): (i32, i32)) -> Option<u8> {
    (num % den == 0).then(|| (num / den) as u8)
}

fn height_sample(scratch: &Scratch, x: i32, y: i32, z: i32, kind: usize) -> Option<u8> {
    let index = border_index(x, y, z);
    let cell = scratch.fluid[index];
    if same_fluid(cell, kind) {
        let above = scratch.fluid[border_index(x, y + 1, z)];
        return Some(if same_fluid(above, kind) {
            FLUID_FULL
        } else {
            cell & FLUID_AMOUNT
        });
    }
    (!scratch.occludes[index]).then_some(0)
}

fn corner_height(
    scratch: &Scratch,
    [x, y, z]: [i32; 3],
    kind: usize,
    own: u8,
    dx: i32,
    dz: i32,
) -> (i32, i32) {
    let full = (i32::from(FLUID_FULL), 1);
    let a = height_sample(scratch, x + dx, y, z, kind);
    let b = height_sample(scratch, x, y, z + dz, kind);
    if a == Some(FLUID_FULL) || b == Some(FLUID_FULL) {
        return full;
    }
    let mut num = 0;
    let mut den = 0;
    if a.unwrap_or(0) > 0 || b.unwrap_or(0) > 0 {
        let diagonal = height_sample(scratch, x + dx, y, z + dz, kind);
        if diagonal == Some(FLUID_FULL) {
            return full;
        }
        weigh(&mut num, &mut den, diagonal);
    }
    weigh(&mut num, &mut den, Some(own));
    weigh(&mut num, &mut den, a);
    weigh(&mut num, &mut den, b);
    (num, den)
}

#[inline]
fn weigh(num: &mut i32, den: &mut i32, sample: Option<u8>) {
    let Some(height) = sample else { return };
    let weight = if height >= 8 { 10 } else { 1 };
    *num += i32::from(height) * weight;
    *den += weight;
}

fn flow_angle(scratch: &Scratch, [x, y, z]: [i32; 3], kind: usize, own: u8) -> Option<f32> {
    let ninth = 1.0 / f32::from(FLUID_FULL);
    let own_height = f32::from(own) * ninth;
    let mut flow_x = 0.0f32;
    let mut flow_z = 0.0f32;
    for face in 2..6usize {
        let normal = face_normal(face);
        let side = [x + normal[0], y + normal[1], z + normal[2]];
        let index = border_index(side[0], side[1], side[2]);
        let cell = scratch.fluid[index];
        if cell != 0 && fluid_kind(cell) != kind {
            continue;
        }
        let distance = if cell != 0 {
            own_height - f32::from(cell & FLUID_AMOUNT) * ninth
        } else if scratch.occludes[index] {
            0.0
        } else {
            let below = scratch.fluid[border_index(side[0], side[1] - 1, side[2])];
            if same_fluid(below, kind) {
                own_height - (f32::from(below & FLUID_AMOUNT) * ninth - 8.0 * ninth)
            } else {
                0.0
            }
        };
        flow_x += normal[0] as f32 * distance;
        flow_z += normal[2] as f32 * distance;
    }
    (flow_x != 0.0 || flow_z != 0.0).then(|| flow_z.atan2(flow_x) - std::f32::consts::FRAC_PI_2)
}

pub(super) fn greedy(catalog: &[BlockInfo], scratch: &mut Scratch, sink: &mut Sink) {
    for kind in 0..FLUID_KINDS {
        if scratch.fluid_kinds >> kind & 1 == 0 {
            continue;
        }
        for face in 0..6usize {
            sweep(
                catalog,
                scratch,
                sink,
                Columns::Fluid(kind),
                face,
                group_face(kind, face),
                face_attr,
            );
        }
    }
}

#[inline]
fn group_face(kind: usize, face: usize) -> u64 {
    if kind == fluid_kind(FLUID_LAVA) {
        face as u64
    } else {
        FACE_NONE as u64
    }
}

fn face_attr(
    catalog: &[BlockInfo],
    scratch: &Scratch,
    local: [i32; 3],
    face: usize,
) -> Option<(u8, u32)> {
    let here = border_index(local[0], local[1], local[2]);
    let state = scratch.states[here] as usize;
    let fluid = catalog[state].fluid?;
    let drop = scratch.flat_drop[section_cell(local[0], local[1], local[2])];

    if scratch.cover[here] >> face & 1 == 1 {
        return None;
    }
    let normal = face_normal(face);
    let front = [
        local[0] + normal[0],
        local[1] + normal[1],
        local[2] + normal[2],
    ];
    let front_index = border_index(front[0], front[1], front[2]);
    let front_cover = scratch.cover[front_index];
    if front_cover >> (face ^ 1) & 1 == 1 && !(face == 1 && drop > 0) {
        return None;
    }

    let vertical = if face == 0 {
        border_index(local[0], local[1] - 1, local[2])
    } else {
        border_index(local[0], local[1] + 1, local[2])
    };
    let mine = scratch.light[here] as u32;
    let other = scratch.light[vertical] as u32;
    let block_light = (mine >> 4).max(other >> 4).max(catalog[state].emission as u32);
    let sky_light = (mine & 0xf).max(other & 0xf);

    let sprite = if face < 2 {
        fluid.still
    } else {
        side_sprite(fluid, front_cover)
    };

    let mut words = [0u32; 1];
    FACE_LAYER.set(&mut words, sprite.layer as u64);
    FACE_ARRAY.set(&mut words, sprite.array as u64);
    if !fluid.lava {
        FACE_TINT.set(&mut words, TintKind::Water as u64 + 1);
    }
    FACE_BLOCK_LIGHT.set(&mut words, block_light as u64);
    FACE_SKY_LIGHT.set(&mut words, sky_light as u64);
    FACE_AO.set(&mut words, FACE_AO.max());
    if face >= 2 {
        FACE_FLUID.set(&mut words, 1);
    }

    let pass = if fluid.lava { Pass::Solid } else { Pass::Translucent };
    Some((pass as u8 | drop << PASS_KEY_BITS | FLUID_KEY, words[0]))
}

#[inline]
fn side_sprite(fluid: Fluid, front_cover: u8) -> SpriteRef {
    match fluid.overlay {
        Some(overlay) if front_cover & COVER_SEE_THROUGH != 0 => overlay,
        _ => fluid.flow,
    }
}

pub(super) fn models(catalog: &[BlockInfo], scratch: &mut Scratch, local_section: u32) {
    let sloped = std::mem::take(&mut scratch.sloped);
    for cell in &sloped {
        let [x, y, z] = cell.cell;
        let here = border_index(x, y, z);
        let state = scratch.states[here] as usize;
        let Some(fluid) = catalog[state].fluid else {
            continue;
        };
        let kind = fluid_kind(scratch.fluid[here]);
        let emission = catalog[state].emission as u32;
        let pass = if fluid.lava { Pass::Solid } else { Pass::Translucent } as usize;
        let tint = if fluid.lava { 0 } else { TintKind::Water as u32 + 1 };
        let out = &mut scratch.complex_by_pass[pass][UNGROUPED];

        let mut corners = cell.corners;
        let facing = |face: usize| {
            let normal = face_normal(face);
            border_index(x + normal[0], y + normal[1], z + normal[2])
        };
        let open = |face: usize| {
            let index = facing(face);
            scratch.cover[here] >> face & 1 == 0
                && !same_fluid(scratch.fluid[index], kind)
                && scratch.cover[index] >> (face ^ 1) & 1 == 0
        };
        let lowest = corners.iter().copied().fold(f32::INFINITY, f32::min);
        let above = facing(1);
        let up = scratch.cover[here] >> 1 & 1 == 0
            && !same_fluid(scratch.fluid[above], kind)
            && (scratch.cover[above] & 1 == 0 || lowest < 1.0);
        let down = open(0);
        let bottom = if down { FLUID_INSET } else { 0.0 };

        let light = |a: usize, b: usize| {
            let (a, b) = (scratch.light[a] as u32, scratch.light[b] as u32);
            ((a >> 4).max(b >> 4).max(emission), (a & 0xf).max(b & 0xf))
        };
        let side_light = light(here, above);

        let (fx, fy, fz) = (x as f32, y as f32, z as f32);
        if up {
            corners = corners.map(|corner| corner - FLUID_INSET);
            let [nw, sw, se, ne] = corners;
            let (sprite, uvs) = match cell.flow {
                None => (fluid.still, [[0.0, 0.0], [0.0, 1.0], [1.0, 1.0], [1.0, 0.0]]),
                Some(angle) => {
                    let (sin, cos) = (angle.sin() * 0.25, angle.cos() * 0.25);
                    (
                        fluid.flow,
                        [
                            [0.5 - cos - sin, 0.5 - cos + sin],
                            [0.5 - cos + sin, 0.5 + cos + sin],
                            [0.5 + cos + sin, 0.5 + cos - sin],
                            [0.5 + cos - sin, 0.5 - cos - sin],
                        ],
                    )
                }
            };
            model::push(
                out,
                &model::Quad {
                    positions: [
                        [fx, fy + nw, fz],
                        [fx, fy + sw, fz + 1.0],
                        [fx + 1.0, fy + se, fz + 1.0],
                        [fx + 1.0, fy + ne, fz],
                    ],
                    uvs,
                    shade: [SHADE_UP; 4],
                    light: light(here, above),
                    tint,
                    sprite,
                },
                local_section,
            );
        }

        if down {
            model::push(
                out,
                &model::Quad {
                    positions: [
                        [fx, fy + bottom, fz],
                        [fx + 1.0, fy + bottom, fz],
                        [fx + 1.0, fy + bottom, fz + 1.0],
                        [fx, fy + bottom, fz + 1.0],
                    ],
                    uvs: [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]],
                    shade: [SHADE_DOWN; 4],
                    light: light(border_index(x, y - 1, z), here),
                    tint,
                    sprite: fluid.still,
                },
                local_section,
            );
        }

        for face in 2..6usize {
            if !open(face) {
                continue;
            }
            let [north_west, south_west, south_east, north_east] = corners;
            let (c0, c1, x0, z0, x1, z1) = match face {
                2 => (north_west, north_east, fx, fz + FLUID_INSET, fx + 1.0, fz + FLUID_INSET),
                3 => (
                    south_east,
                    south_west,
                    fx + 1.0,
                    fz + 1.0 - FLUID_INSET,
                    fx,
                    fz + 1.0 - FLUID_INSET,
                ),
                4 => (south_west, north_west, fx + FLUID_INSET, fz + 1.0, fx + FLUID_INSET, fz),
                _ => (
                    north_east,
                    south_east,
                    fx + 1.0 - FLUID_INSET,
                    fz,
                    fx + 1.0 - FLUID_INSET,
                    fz + 1.0,
                ),
            };
            model::push(
                out,
                &model::Quad {
                    positions: [
                        [x0, fy + c0, z0],
                        [x1, fy + c1, z1],
                        [x1, fy + bottom, z1],
                        [x0, fy + bottom, z0],
                    ],
                    uvs: [
                        [0.0, (1.0 - c0) * 0.5],
                        [0.5, (1.0 - c1) * 0.5],
                        [0.5, 0.5],
                        [0.0, 0.5],
                    ],
                    shade: [if face < 4 { SHADE_NORTH_SOUTH } else { SHADE_EAST_WEST }; 4],
                    light: side_light,
                    tint,
                    sprite: side_sprite(fluid, scratch.cover[facing(face)]),
                },
                local_section,
            );
        }
    }
    scratch.sloped = sloped;
}

#[cfg(test)]
mod tests {
    use super::{FLUID_FULL, drop_steps};
    use crate::anvil::{Palette, SECTION_SIZE, World, one_section_region};
    use crate::atlas::SpriteRef;
    use crate::bake::Dir;
    use crate::blocks::{BlockInfo, CubeFace, Fluid, Pass};
    use crate::mesh::model::fixed;
    use crate::mesh::{Scratch, mesh_render_region};
    use crate::pack::{
        FACE_LAYER, MODEL_STEPS, QUAD_DROP, QUAD_FACE, QUAD_H, QUAD_W, RegionGrid,
        SECTION_FACE_TABLE,
    };

    fn water(amount: u8) -> Fluid {
        Fluid {
            lava: false,
            amount,
            still: SpriteRef::default(),
            flow: SpriteRef::default(),
            overlay: None,
        }
    }

    fn catalog(palette: &Palette) -> Vec<BlockInfo> {
        (0..palette.states.len()).map(|_| BlockInfo::default()).collect()
    }

    fn state_id(palette: &Palette, name: &str) -> usize {
        palette.states.iter().position(|state| state.name == name).unwrap()
    }

    #[test]
    fn the_two_fluid_paths_put_a_surface_in_the_same_place() {
        for ninths in 0..=FLUID_FULL {
            let merged = MODEL_STEPS - f32::from(drop_steps(ninths));
            let model = fixed(f32::from(ninths) / f32::from(FLUID_FULL)) as f32 - fixed(0.0) as f32;
            assert_eq!(merged, model, "{ninths} ninths of a block lands in two places");
        }
    }

    #[test]
    fn a_flat_sea_merges_into_one_quad_a_ninth_below_the_block_top() {
        let mut palette = Palette::new();
        let mut world = World::new([0, 0], [1, 1]);
        world.insert(&mut palette, [0, 0], one_section_region("minecraft:water"));
        let mut blocks = catalog(&palette);
        blocks[state_id(&palette, "minecraft:water")].fluid = Some(water(8));

        let grid = RegionGrid::covering(world.sections);
        let mut scratch = Scratch::new();
        let mut surfaces = Vec::new();
        let mut models = 0;
        for region in 0..grid.len() {
            let batch = mesh_render_region(&world, &blocks, grid, region, &mut scratch);
            models += batch.model_quads();
            for quad in &batch.simple {
                if QUAD_FACE.read(quad) == 1 {
                    surfaces.push((
                        QUAD_W.read(quad) + 1,
                        QUAD_H.read(quad) + 1,
                        QUAD_DROP.read(quad),
                    ));
                }
            }
        }

        let interior = SECTION_SIZE as u64 - 2;
        assert_eq!(
            surfaces,
            [(interior, interior, drop_steps(8) as u64)],
            "a flat sea did not come out as a single sunk quad"
        );
        assert!(models > 0, "the rim of the sea slopes away and has to be modelled");
    }

    #[test]
    fn a_waterlogged_block_hides_the_fluid_faces_it_covers() {
        let upward = |sturdy: u8| {
            let mut palette = Palette::new();
            let mut world = World::new([0, 0], [1, 1]);
            world.insert(&mut palette, [0, 0], one_section_region("minecraft:oak_slab"));
            let id = state_id(&palette, "minecraft:oak_slab");
            let mut blocks = catalog(&palette);
            blocks[id].sturdy = sturdy;
            blocks[id].fluid = Some(water(8));

            let grid = RegionGrid::covering(world.sections);
            let mut scratch = Scratch::new();
            (0..grid.len())
                .map(|region| mesh_render_region(&world, &blocks, grid, region, &mut scratch))
                .flat_map(|batch| batch.simple.into_iter())
                .filter(|quad| QUAD_FACE.read(quad) == Dir::Up as u64)
                .count()
        };

        assert_eq!(upward(0), 1, "open water shows its surface");
        assert_eq!(
            upward(1 << Dir::Up as u8),
            0,
            "a block that closes its own top must hide the water under it"
        );
    }

    #[test]
    fn water_against_glass_takes_the_overlay_texture() {
        const WATER: usize = 0;
        const GLASS: usize = 1;
        let mut palette = Palette::new();
        let mut world = World::new([0, 0], [1, 1]);
        world.insert(
            &mut palette,
            [0, 0],
            crate::anvil::one_section_region_of(
                &["minecraft:water", "minecraft:glass"],
                |x, _, _| if x < 8 { WATER } else { GLASS },
            ),
        );

        let sprite = |layer: u16| SpriteRef { array: 0, layer };
        let mut blocks = catalog(&palette);
        blocks[state_id(&palette, "minecraft:water")].fluid = Some(Fluid {
            still: sprite(2),
            flow: sprite(3),
            overlay: Some(sprite(4)),
            ..water(8)
        });
        blocks[state_id(&palette, "minecraft:glass")].cube = Some([CubeFace {
            sprite: sprite(1),
            pass: Pass::Translucent as u8,
            tinted: false,
        }; 6]);

        let grid = RegionGrid::covering(world.sections);
        let mut scratch = Scratch::new();
        let mut sprites = [0usize; 5];
        for region in 0..grid.len() {
            let batch = mesh_render_region(&world, &blocks, grid, region, &mut scratch);
            for attr in batch.faces.iter().skip(SECTION_FACE_TABLE) {
                sprites[FACE_LAYER.get(*attr as u64) as usize] += 1;
            }
        }

        assert_eq!(
            sprites[4],
            (SECTION_SIZE - 1) * SECTION_SIZE,
            "the water did not meet the glass with its overlay"
        );
        assert!(sprites[3] > 0, "water away from the glass keeps the flowing texture");
    }
}
