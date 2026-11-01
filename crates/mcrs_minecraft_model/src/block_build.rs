use crate::atlas::{Opacity, SpriteRegistry};
use crate::bake::{self, Dir};
use crate::block_catalog::{BlockStateKey, cube_corners};
use crate::model::Pack;
use bevy::math::Vec3;
use mcrs_minecraft_block::block_keys::Block;
use mcrs_minecraft_block::definition::{BlockStateData, BlockStateFlags};
use mcrs_minecraft_core::voxel_shape::Aabb;
use mcrs_minecraft_mesh::ambient::Neighbour;
use mcrs_minecraft_mesh::block::{
    BlockInfo, CORNER_UV, CubeFace, FACE_AXES, FaceShapes, Fluid, ModelQuad, Pass, SideCells,
};
use mcrs_minecraft_mesh::pack::MODEL_STEPS;
use mcrs_minecraft_mesh::tint::Tint;

fn fluid_of(
    pack: &Pack,
    block: Block,
    state: &BlockStateKey,
    data: &BlockStateData,
    sprites: &mut SpriteRegistry,
) -> Result<Option<Fluid>, String> {
    let prop = |key: &str| {
        state
            .props
            .iter()
            .find(|(name, _)| name == key)
            .map(|(_, value)| value.as_str())
    };
    let (lava, amount) = match block {
        Block::Water | Block::Lava => {
            let fluid = data
                .fluid
                .ok_or_else(|| format!("{} states no fluid", state.name))?;
            (block == Block::Lava, fluid.level)
        }
        Block::BubbleColumn
        | Block::Kelp
        | Block::KelpPlant
        | Block::Seagrass
        | Block::TallSeagrass => (false, 8),
        _ if prop("waterlogged") == Some("true") => (false, 8),
        _ => return Ok(None),
    };
    let (still, flow) = if lava {
        ("minecraft:block/lava_still", "minecraft:block/lava_flow")
    } else {
        ("minecraft:block/water_still", "minecraft:block/water_flow")
    };
    let overlay = match lava {
        true => None,
        false => Some(sprites.intern(pack, "minecraft:block/water_overlay")?),
    };
    Ok(Some(Fluid {
        lava,
        amount,
        still: sprites.intern(pack, still)?,
        flow: sprites.intern(pack, flow)?,
        overlay,
    }))
}

const PAST_QUADS: &str = "a face names a quad past the baked model";
const PAST_SPRITES: &str = "a quad names a sprite past the baked model's table";

pub(crate) fn build_one(
    pack: &Pack,
    block: Block,
    state: &BlockStateKey,
    data: &BlockStateData,
    occlusion: &[Aabb],
    sprites: &mut SpriteRegistry,
) -> Result<BlockInfo, String> {
    if matches!(block, Block::Air | Block::CaveAir | Block::VoidAir) {
        return Ok(BlockInfo::default());
    }

    let emission = data.light_emission;
    let fluid = fluid_of(pack, block, state, data, sprites)?;

    let solid_render = data.flags.contains(BlockStateFlags::IS_SOLID_RENDER);
    let neighbour = Neighbour {
        full_block: data
            .flags
            .contains(BlockStateFlags::IS_COLLISION_SHAPE_FULL_BLOCK),
        solid_render,
        light_opaque: solid_render && data.light_dampening != 0,
    };
    let emissive = data.flags.contains(BlockStateFlags::EMISSIVE_RENDERING);
    let faces = face_shapes(occlusion);

    let baked = bake::bake(pack, &state.name, &state.pairs())?;
    if baked.quads.is_empty() {
        return Ok(BlockInfo {
            fluid,
            neighbour,
            emission,
            emissive,
            faces,
            ..BlockInfo::default()
        });
    }

    let mut layers: Vec<u16> = Vec::with_capacity(baked.sprites.len());
    for sprite in &baked.sprites {
        layers.push(sprites.intern(pack, sprite)?);
    }

    let sturdy = sturdy_faces(&baked.quads, &layers, sprites);

    let (mut cube_faces, mut extras) = split_cube(&baked.quads);
    let mut drop = 0;
    if cube_faces.is_none()
        && let Some((top, bottom, lowered)) = split_lowered(&baked.quads)
    {
        cube_faces = Some(std::array::from_fn(|dir| {
            if dir == Dir::Down as usize {
                bottom
            } else {
                top
            }
        }));
        extras.retain(|&index| index != top && index != bottom);
        drop = lowered;
    }
    let mut cube = None;
    let mut occludes = false;
    let mut self_culls = false;
    if let Some(faces) = cube_faces {
        let mut built = [CubeFace::default(); 6];
        let mut worst = Pass::Solid;
        for (&index, face) in faces.iter().zip(&mut built) {
            let quad = baked.quads.get(index).ok_or(PAST_QUADS)?;
            let interned = *layers.get(quad.sprite).ok_or(PAST_SPRITES)?;
            let pass = Pass::from(sprites.opacity(interned));
            worst = worst.max(pass);
            *face = CubeFace {
                sprite: interned,
                pass: pass as u8,
                tint: tint_of(state, quad.tint),
            };
        }
        cube = Some(built);
        occludes = drop == 0 && worst == Pass::Solid;
        self_culls = drop == 0 && worst == Pass::Translucent;
    }

    let mut quads = Vec::with_capacity(extras.len());
    for &index in &extras {
        let quad = baked.quads.get(index).ok_or(PAST_QUADS)?;
        let interned = *layers.get(quad.sprite).ok_or(PAST_SPRITES)?;
        quads.push(ModelQuad {
            positions: quad.positions,
            uvs: quad.uvs,
            cull: quad.cull,
            facing: quad.dir,
            face: face_group(&quad.positions),
            sprite: interned,
            pass: Pass::from(sprites.opacity(interned)),
            shade: quad.shade,
            tint: tint_of(state, quad.tint),
        });
    }

    Ok(BlockInfo {
        cube,
        faces,
        drop,
        quads,
        occludes,
        self_culls,
        sturdy,
        emission,
        emissive,
        fluid,
        neighbour,
        ambient_occlusion: baked.ambient_occlusion && emission == 0,
    })
}

const FACE_GRID: usize = 16;

/// Vanilla's `VoxelShape.getFaceShape` per side, on a sixteenth grid: the boxes that reach the
/// side, seen along its normal.
fn face_shapes(occlusion: &[Aabb]) -> Option<Box<FaceShapes>> {
    const EDGE: f32 = 1e-6;
    if occlusion.is_empty() {
        return None;
    }
    let mut faces = FaceShapes {
        outer: [[0; FACE_GRID]; 6],
        inner: [[0; FACE_GRID]; 6],
    };
    let sides = FACE_AXES
        .iter()
        .zip(faces.outer.iter_mut().zip(&mut faces.inner));
    for (&[normal, sign, ..], (outer, inner)) in sides {
        let normal = usize::from(normal);
        let (rows, columns) = tangents(normal);
        for aabb in occlusion {
            let (min, max) = (aabb.min.to_array(), aabb.max.to_array());
            let reaches = if sign == 1 {
                along(max, normal) >= 1.0 - EDGE
            } else {
                along(min, normal) <= EDGE
            };
            if reaches {
                let row_span = (along(min, rows), along(max, rows));
                let column_span = (along(min, columns), along(max, columns));
                mark(outer, row_span, column_span, true);
                mark(inner, row_span, column_span, false);
            }
        }
    }
    Some(Box::new(faces))
}

/// Sets the cells a box's side spans: all it touches when rounding out, only those it fills when
/// rounding in.
fn mark(side: &mut SideCells, rows: (f32, f32), columns: (f32, f32), out: bool) {
    let steps = FACE_GRID as f32;
    let span = |(low, high): (f32, f32)| {
        let (low, high) = if out {
            ((low * steps + 1e-4).floor(), (high * steps - 1e-4).ceil())
        } else {
            ((low * steps - 1e-4).ceil(), (high * steps + 1e-4).floor())
        };
        low.clamp(0.0, steps) as usize..high.clamp(0.0, steps) as usize
    };
    let columns = span(columns);
    if columns.is_empty() {
        return;
    }
    let bits = ((1u32 << columns.len()) - 1) << columns.start;
    if let Some(cells) = side.get_mut(span(rows)) {
        for cell in cells {
            *cell |= bits as u16;
        }
    }
}

fn sturdy_faces(quads: &[bake::BakedQuad], layers: &[u16], sprites: &SpriteRegistry) -> u8 {
    let mut sides = [[0u16; FACE_GRID]; Dir::all().len()];
    for quad in quads {
        let Some(dir) = quad.cull else { continue };
        let Some(&layer) = layers.get(quad.sprite) else {
            continue;
        };
        if sprites.opacity(layer) != Opacity::Solid {
            continue;
        }
        if let Some(side) = sides.get_mut(dir as usize) {
            cover_face(side, &quad.positions, dir);
        }
    }
    let mut mask = 0;
    for (dir, side) in Dir::all().into_iter().zip(&sides) {
        if side.iter().all(|row| *row == u16::MAX) {
            mask |= 1 << dir as u8;
        }
    }
    mask
}

fn cover_face(side: &mut [u16; FACE_GRID], positions: &[Vec3; 4], dir: Dir) {
    let Some(&[normal, sign, ..]) = FACE_AXES.get(dir as usize) else {
        return;
    };
    let normal = usize::from(normal);
    let plane = if sign == 1 { 1.0 } else { 0.0 };
    let mut low = Vec3::INFINITY;
    let mut high = Vec3::NEG_INFINITY;
    for &position in positions {
        if (along(position.to_array(), normal) - plane).abs() > 1e-4 {
            return;
        }
        low = low.min(position);
        high = high.max(position);
    }
    let (low, high) = (low.to_array(), high.to_array());
    let steps = FACE_GRID as f32;
    let cell = |value: f32, round_up: bool| {
        let scaled = value * steps;
        let rounded = if round_up {
            scaled.ceil()
        } else {
            scaled.floor()
        };
        rounded.clamp(0.0, steps) as usize
    };
    let (rows, columns) = tangents(normal);
    let span = cell(along(low, columns), true)..cell(along(high, columns), false);
    if span.is_empty() {
        return;
    }
    let bits = (((1u32 << span.len()) - 1) << span.start) as u16;
    let (first_row, end_row) = (cell(along(low, rows), true), cell(along(high, rows), false));
    for covered in side.iter_mut().take(end_row).skip(first_row) {
        *covered |= bits;
    }
}

/// The coordinate of `point` along `axis`, 0 for x, 1 for y and 2 for z.
fn along(point: [f32; 3], axis: usize) -> f32 {
    let [x, y, z] = point;
    match axis {
        0 => x,
        1 => y,
        _ => z,
    }
}

/// The two axes across a face whose normal runs along `normal`, in axis order.
fn tangents(normal: usize) -> (usize, usize) {
    match normal {
        0 => (1, 2),
        1 => (0, 2),
        _ => (0, 1),
    }
}

const DIAGONALS: [[f32; 2]; 4] = [[1.0, 1.0], [1.0, -1.0], [-1.0, 1.0], [-1.0, -1.0]];

fn face_group(positions: &[Vec3; 4]) -> Option<u8> {
    let normal = (positions[1] - positions[0])
        .cross(positions[2] - positions[0])
        .normalize_or_zero();
    if let Some(dir) = Dir::all()
        .into_iter()
        .find(|dir| normal.dot(dir.normal().as_vec3()) >= 1.0 - 1e-4)
    {
        return Some(dir as u8);
    }
    let diagonal = DIAGONALS
        .iter()
        .position(|&[x, z]| normal.dot(Vec3::new(x, 0.0, z).normalize()) >= 1.0 - 1e-4)?;
    Some(Dir::all().len() as u8 + diagonal as u8)
}

/// Vanilla's `BlockColors`: the colour each block multiplies into the faces that name a tint
/// layer. A layer past the block's list, or a block with none, is left untinted.
fn tint_of(state: &BlockStateKey, layer: Option<u32>) -> Tint {
    let Some(layer) = layer else {
        return Tint::None;
    };
    let prop = |key: &str| {
        state
            .props
            .iter()
            .find(|(name, _)| name == key)
            .and_then(|(_, value)| value.parse::<u8>().ok())
            .unwrap_or(0)
    };
    let name = state.name.strip_prefix("minecraft:").unwrap_or(&state.name);
    let layers: &[Tint] = match name {
        "large_fern" | "tall_grass" | "fern" | "short_grass" | "potted_fern" | "bush"
        | "grass_block" | "sugar_cane" => &[Tint::Grass],
        "pink_petals" | "wildflowers" => &[Tint::None, Tint::Grass],
        "spruce_leaves" => &[Tint::SPRUCE_LEAVES],
        "birch_leaves" => &[Tint::BIRCH_LEAVES],
        "oak_leaves" | "jungle_leaves" | "acacia_leaves" | "dark_oak_leaves" | "vine"
        | "mangrove_leaves" => &[Tint::Foliage],
        "leaf_litter" => &[Tint::DryFoliage],
        "water_cauldron" => &[Tint::Water],
        "attached_melon_stem" | "attached_pumpkin_stem" => &[Tint::ATTACHED_STEM],
        "lily_pad" => &[Tint::LILY_PAD],
        "melon_stem" | "pumpkin_stem" => &[Tint::stem(prop("age"))],
        "redstone_wire" => &[Tint::redstone(prop("power"))],
        _ => &[],
    };
    layers.get(layer as usize).copied().unwrap_or(Tint::None)
}

fn split_cube(quads: &[bake::BakedQuad]) -> (Option<[usize; 6]>, Vec<usize>) {
    let mut faces = [usize::MAX; 6];
    let mut extras = Vec::new();
    for (index, quad) in quads.iter().enumerate() {
        match faces.get_mut(quad.dir as usize) {
            Some(slot) if *slot == usize::MAX && is_cube_face(quad) => *slot = index,
            _ => extras.push(index),
        }
    }
    if faces.contains(&usize::MAX) {
        return (None, (0..quads.len()).collect());
    }
    (Some(faces), extras)
}

/// A box as wide as its cell standing on the cell's floor, its top and bottom textured the way a
/// cube's are: they tile across neighbours like cube faces, the top dropped below the cell's by
/// a whole number of `MODEL_STEPS`. Answers the top, the bottom and the drop.
fn split_lowered(quads: &[bake::BakedQuad]) -> Option<(usize, usize, u8)> {
    let bottom = quads
        .iter()
        .position(|quad| quad.dir == Dir::Down && is_cube_face(quad))?;
    quads.iter().enumerate().find_map(|(index, quad)| {
        if quad.dir != Dir::Up || quad.cull.is_some() || !cube_uvs(quad) {
            return None;
        }
        let height = quad.positions[0].y;
        let drop = (1.0 - height) * MODEL_STEPS;
        if !(1.0..MODEL_STEPS).contains(&drop.round()) || (drop - drop.round()).abs() > 1e-3 {
            return None;
        }
        let lowered =
            mcrs_minecraft_mesh::ambient::corners(Dir::Up, Vec3::ZERO, Vec3::new(1.0, height, 1.0))
                .iter()
                .zip(&quad.positions)
                .all(|(at, position)| position.distance_squared(*at) <= 1e-6);
        lowered.then_some((index, bottom, drop.round() as u8))
    })
}

fn cube_uvs(quad: &bake::BakedQuad) -> bool {
    quad.uvs
        .iter()
        .zip(&CORNER_UV)
        .all(|(&uv, &want)| near_uv(uv, want))
}

fn is_cube_face(quad: &bake::BakedQuad) -> bool {
    if quad.cull != Some(quad.dir) {
        return false;
    }
    let corners = quad.positions.iter().zip(cube_corners(quad.dir));
    corners
        .zip(quad.uvs.iter().zip(&CORNER_UV))
        .all(|((position, corner), (&uv, &want))| {
            position.distance_squared(corner) <= 1e-6 && near_uv(uv, want)
        })
}

fn near_uv([u, v]: [f32; 2], [want_u, want_v]: [f32; 2]) -> bool {
    (u - want_u).abs() <= 1e-4 && (v - want_v).abs() <= 1e-4
}

#[cfg(test)]
mod tests {

    mod jar {
        use super::*;

        #[test]
        fn block_states_bake_from_the_corpus() {
            a_fluid_level_reads_back_as_the_height_vanilla_gives_it();
            ambient_occlusion_reads_its_neighbours_from_the_corpus();
            a_block_lights_the_mesh_by_what_the_corpus_says_it_emits();
            a_block_closes_the_sides_its_own_geometry_covers();
            a_face_hides_behind_a_neighbour_whose_shape_covers_it();
            a_box_lower_than_its_cell_meshes_its_top_and_bottom_as_cube_faces();
            cube_uv_matches_the_vanilla_bake();
            a_rotated_log_is_not_greedy_meshable();
        }
    }

    fn a_fluid_level_reads_back_as_the_height_vanilla_gives_it() {
        let ninths = [8u8, 7, 6, 5, 4, 3, 2, 1, 8, 8, 8, 8, 8, 8, 8, 8];
        for (level, height) in ninths.iter().enumerate() {
            let info = bake_state("minecraft:water", &[("level", &level.to_string())]);
            assert_eq!(
                info.fluid.expect("water is a fluid").amount,
                *height,
                "level {level}"
            );
        }
    }

    #[test]
    fn each_block_tints_its_layers_the_way_vanilla_registers_them() {
        use mcrs_minecraft_mesh::tint::Tint;
        let key = |name: &str, props: &[(&str, &str)]| BlockStateKey {
            name: format!("minecraft:{name}"),
            props: props
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect(),
        };
        let tint = |name, props, layer| super::tint_of(&key(name, props), layer);
        assert_eq!(tint("cherry_leaves", &[], Some(0)), Tint::None);
        assert_eq!(tint("azalea_leaves", &[], Some(0)), Tint::None);
        assert_eq!(tint("oak_leaves", &[], Some(0)), Tint::Foliage);
        assert_eq!(tint("dark_oak_leaves", &[], Some(0)), Tint::Foliage);
        assert_eq!(tint("spruce_leaves", &[], Some(0)), Tint::SPRUCE_LEAVES);
        assert_eq!(tint("birch_leaves", &[], Some(0)), Tint::BIRCH_LEAVES);
        assert_eq!(tint("leaf_litter", &[], Some(0)), Tint::DryFoliage);
        assert_eq!(
            tint("grass_block", &[], None),
            Tint::None,
            "an untinted face"
        );
        assert_eq!(tint("pink_petals", &[], Some(0)), Tint::None);
        assert_eq!(tint("pink_petals", &[], Some(1)), Tint::Grass);
        assert_eq!(
            tint("redstone_wire", &[("power", "7")], Some(0)),
            Tint::redstone(7)
        );
        assert_eq!(
            tint("pumpkin_stem", &[("age", "3")], Some(0)),
            Tint::stem(3)
        );
        assert_eq!(tint("stone", &[], Some(0)), Tint::None);
    }

    fn ambient_occlusion_reads_its_neighbours_from_the_corpus() {
        let stone = bake_state("minecraft:stone", &[]);
        assert!(stone.neighbour.full_block && stone.neighbour.light_opaque);

        let glass = bake_state("minecraft:glass", &[]);
        assert!(
            glass.neighbour.full_block,
            "glass still darkens its neighbours"
        );
        assert!(!glass.neighbour.light_opaque, "light passes through glass");

        let stairs = bake_state(
            "minecraft:oak_stairs",
            &[
                ("facing", "north"),
                ("half", "bottom"),
                ("shape", "straight"),
                ("waterlogged", "false"),
            ],
        );
        assert!(stairs.ambient_occlusion);
        assert!(!stairs.neighbour.full_block);

        assert!(
            !bake_state("minecraft:torch", &[]).ambient_occlusion,
            "a block that emits light is never occluded"
        );
    }

    fn a_block_lights_the_mesh_by_what_the_corpus_says_it_emits() {
        assert_eq!(bake_state("minecraft:torch", &[]).emission, 14);
        assert_eq!(bake_state("minecraft:glowstone", &[]).emission, 15);
        assert_eq!(bake_state("minecraft:stone", &[]).emission, 0);
    }

    use super::{Block, BlockStateKey, Pack, cube_corners, face_group, split_cube};
    use crate::atlas::SpriteRegistry;
    use crate::bake::{self, Dir};
    use bevy::math::Vec3;

    fn bake_state(name: &str, props: &[(&str, &str)]) -> super::BlockInfo {
        let state = BlockStateKey {
            name: name.to_string(),
            props: props
                .iter()
                .map(|(key, value)| (key.to_string(), value.to_string()))
                .collect(),
        };
        let corpus = crate::block_catalog::corpus();
        let mut id = corpus.default_state(name).unwrap();
        for (key, value) in props {
            id = corpus
                .owner(id)
                .unwrap()
                .with_text(id, key, value)
                .unwrap_or_else(|| panic!("{name} has no {key}={value}"));
        }
        super::build_one(
            Pack::corpus(),
            corpus
                .block_index(id)
                .and_then(Block::from_id)
                .expect("a corpus block is a registry entry"),
            &state,
            corpus.state(id),
            corpus.shape(corpus.state(id).occlusion_shape),
            &mut SpriteRegistry::default(),
        )
        .unwrap_or_else(|reason| panic!("{name} does not bake: {reason}"))
    }

    fn closed(name: &str, props: &[(&str, &str)]) -> Vec<&'static str> {
        let sturdy = bake_state(name, props).sturdy;
        Dir::all()
            .into_iter()
            .filter(|dir| sturdy >> *dir as u8 & 1 == 1)
            .map(|dir| dir.name())
            .collect()
    }

    fn a_block_closes_the_sides_its_own_geometry_covers() {
        assert_eq!(
            closed("minecraft:stone", &[]),
            ["down", "up", "north", "south", "west", "east"],
            "a full cube closes everything"
        );
        assert_eq!(
            closed("minecraft:oak_slab", &[("type", "bottom")]),
            ["down"],
            "a slab closes the side it sits on and nothing else"
        );
        assert_eq!(closed("minecraft:oak_slab", &[("type", "top")]), ["up"]);
        assert_eq!(
            closed(
                "minecraft:oak_stairs",
                &[
                    ("facing", "east"),
                    ("half", "bottom"),
                    ("shape", "straight")
                ],
            ),
            ["down", "east"],
        );
        assert_eq!(
            closed(
                "minecraft:oak_stairs",
                &[("facing", "north"), ("half", "top"), ("shape", "straight")],
            ),
            ["up", "north"],
        );
        assert!(
            closed("minecraft:oak_fence", &[]).is_empty(),
            "a fence post covers no side of its block"
        );
        assert!(
            closed("minecraft:glass", &[]).is_empty(),
            "glass covers every side and hides none of them, which is what the fluid overlay is for"
        );
        assert!(
            closed(
                "minecraft:oak_leaves",
                &[("persistent", "false"), ("distance", "7")]
            )
            .is_empty(),
            "leaves are a cube full of holes and hide nothing"
        );
    }

    #[test]
    fn a_quad_joins_the_face_group_it_squarely_points_along() {
        for dir in Dir::all() {
            let face = cube_corners(dir);
            assert_eq!(
                face_group(&face),
                Some(dir as u8),
                "the {} face of a cube",
                dir.name()
            );
        }

        let pane = [
            Vec3::new(0.2, 1.0, 0.2),
            Vec3::new(0.2, 0.0, 0.2),
            Vec3::new(0.8, 0.0, 0.8),
            Vec3::new(0.8, 1.0, 0.8),
        ];
        let normal = (pane[1] - pane[0]).cross(pane[2] - pane[0]);
        assert!(
            bake::nearest(normal).is_some(),
            "the nearest axis answers even for a pane that faces none of them"
        );
        let group = face_group(&pane).expect("a plant's pane points along a diagonal");
        assert!(
            group >= Dir::all().len() as u8,
            "a pane must not be filed under an axis, it would vanish from one side"
        );
        let mirrored = [pane[3], pane[2], pane[1], pane[0]];
        assert_ne!(
            face_group(&mirrored),
            Some(group),
            "the same pane wound the other way faces the other way"
        );
    }

    fn a_face_hides_behind_a_neighbour_whose_shape_covers_it() {
        use mcrs_minecraft_mesh::block::face_hidden;
        let snow = |layers: &str| bake_state("minecraft:snow", &[("layers", layers)]);
        let (low, same, high) = (snow("2"), snow("2"), snow("5"));
        let (north, east, up, down) = (Dir::North as usize, Dir::East as usize, 1, 0);
        assert!(
            face_hidden(&low, &same, north),
            "snow of one height meets itself"
        );
        assert!(
            face_hidden(&low, &high, east),
            "a deeper drift covers a shallower one's side"
        );
        assert!(
            !face_hidden(&high, &low, east),
            "and not the other way round"
        );
        assert!(
            !face_hidden(&low, &same, up),
            "the top of a layer touches nothing above it"
        );

        let stone = bake_state("minecraft:stone", &[]);
        assert!(
            face_hidden(&low, &stone, down),
            "a full block hides whatever faces it"
        );
        assert!(
            face_hidden(&stone, &low, up),
            "a layer's underside covers the whole top of the block it lies on"
        );

        let grass = bake_state("minecraft:short_grass", &[]);
        assert!(grass.faces.is_none(), "a plant occludes nothing");
        assert!(!face_hidden(&low, &grass, north));
    }

    fn a_box_lower_than_its_cell_meshes_its_top_and_bottom_as_cube_faces() {
        for layers in 1..=7u8 {
            let snow = bake_state("minecraft:snow", &[("layers", &layers.to_string())]);
            assert!(snow.cube.is_some(), "{layers} layers of snow");
            assert_eq!(snow.drop, 32 - 4 * layers, "{layers} layers of snow");
            let baked = bake::bake(
                Pack::corpus(),
                "minecraft:snow",
                &[("layers", &layers.to_string())],
            )
            .expect("snow bakes");
            assert_eq!(
                snow.quads.len(),
                baked.quads.len() - 2,
                "only the top and the bottom leave the model quads"
            );
            assert!(!snow.occludes);
        }
        assert_eq!(
            bake_state("minecraft:snow", &[("layers", "8")]).drop,
            0,
            "a full block of snow"
        );
        assert_eq!(bake_state("minecraft:white_carpet", &[]).drop, 30);
        assert_eq!(
            bake_state("minecraft:oak_slab", &[("type", "bottom")]).drop,
            16
        );
        let top_slab = bake_state("minecraft:oak_slab", &[("type", "top")]);
        assert!(
            top_slab.cube.is_none() && top_slab.drop == 0,
            "a top slab does not stand on its floor"
        );
    }

    fn cube_uv_matches_the_vanilla_bake() {
        let baked = bake::bake(Pack::corpus(), "minecraft:stone", &[]).expect("stone bakes");
        let (faces, extras) = split_cube(&baked.quads);
        let faces = faces.expect("stone is a full cube");
        assert!(extras.is_empty(), "stone has nothing beyond its cube");
        for dir in Dir::all() {
            let quad = &baked.quads[faces[dir as usize]];
            for (corner, position) in quad.positions.iter().enumerate().take(4) {
                assert_eq!(
                    *position,
                    cube_corners(dir)[corner],
                    "{dir:?} corner {corner}"
                );
            }
        }
    }

    fn a_rotated_log_is_not_greedy_meshable() {
        let upright = bake::bake(Pack::corpus(), "minecraft:oak_log", &[("axis", "y")])
            .expect("upright log bakes");
        assert!(split_cube(&upright.quads).0.is_some());

        let sideways = bake::bake(Pack::corpus(), "minecraft:oak_log", &[("axis", "x")])
            .expect("sideways log bakes");
        assert!(
            split_cube(&sideways.quads).0.is_none(),
            "a log turned on its side has rotated UVs and cannot tile"
        );

        let grass = bake::bake(
            Pack::corpus(),
            "minecraft:grass_block",
            &[("snowy", "false")],
        )
        .expect("grass block bakes");
        let (cube, extras) = split_cube(&grass.quads);
        assert!(cube.is_some(), "the grass block hides a full cube");
        assert_eq!(extras.len(), 4, "four tinted side overlays are left over");
    }
}
