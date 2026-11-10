use crate::atlas::{Opacity, SpriteRegistry};
use crate::bake::{BakedBlock, BakedFace, BlockModels};
use crate::bake_error::BakeError;
use crate::block_catalog::{BlockStateKey, cube_corners};
use crate::block_entity_model::Surface;
use crate::resource_set::ResourceSet;
use bevy::math::Vec3;
use mcrs_minecraft_block::block_keys::Block;
use mcrs_minecraft_block::definition::{BlockStateData, BlockStateFlags};
use mcrs_minecraft_core::Direction;
use mcrs_minecraft_core::voxel_shape::Aabb;
use mcrs_minecraft_mesh::block::{
    BlockInfo, CORNER_UV, CubeFace, FACE_AXES, FaceShapes, Fluid, ModelQuad, Pass, SideCells,
};
use mcrs_minecraft_mesh::face_shade::FaceShade;
use mcrs_minecraft_mesh::pack::MODEL_STEPS;
use mcrs_minecraft_mesh::smooth_light::LightState;
use mcrs_minecraft_mesh::state_variants::{Shape, StateVariants, VariantShapes};
use mcrs_minecraft_mesh::tint::Tint;
use mcrs_minecraft_mesh::uv_orientation::UvOrientation;
use mcrs_minecraft_mesh::variant_pick::PartSource;
use mcrs_minecraft_model_file::block_state_file::Variant;
use mcrs_minecraft_model_file::property_table::PropertyTable;

fn fluid_of(
    pack: &ResourceSet,
    block: Block,
    state: &BlockStateKey,
    data: &BlockStateData,
    sprites: &mut SpriteRegistry,
) -> Result<Option<Fluid>, BakeError> {
    let prop = |key: &str| {
        state
            .props
            .iter()
            .find(|(name, _)| name == key)
            .map(|(_, value)| value.as_str())
    };
    let (lava, amount) = match block {
        Block::Water | Block::Lava => {
            let fluid = data.fluid.ok_or_else(|| BakeError::NoFluid {
                block: state.name.clone(),
            })?;
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

pub(crate) fn build_one(
    models: &mut BlockModels<'_>,
    block: Block,
    state: &BlockStateKey,
    properties: &PropertyTable,
    data: &BlockStateData,
    occlusion: &[Aabb],
    sprites: &mut SpriteRegistry,
) -> Result<BlockInfo, BakeError> {
    if matches!(block, Block::Air | Block::CaveAir | Block::VoidAir) {
        return Ok(BlockInfo::default());
    }
    let set = models.set();

    let emission = data.light_emission;
    let fluid = fluid_of(set, block, state, data, sprites)?;

    let light = LightState {
        occlusion_shade: data.occlusion_shade(),
        solid_render: data.flags.contains(BlockStateFlags::IS_SOLID_RENDER),
        light_dampening: data.light_dampening,
        full_collision: data
            .flags
            .contains(BlockStateFlags::IS_COLLISION_SHAPE_FULL_BLOCK),
        emission,
        emissive: data.flags.contains(BlockStateFlags::EMISSIVE_RENDERING),
    };
    let faces = face_shapes(occlusion);

    let values = properties
        .state(&state.pairs())
        .ok_or_else(|| BakeError::NotAState {
            state: state.label(),
        })?;
    let parts = models.parts(&state.name, properties, &values);
    let pick = parts.pick(properties, &values);
    let varies = parts.parts.iter().any(|part| part.len() > 1) && pick.choices().is_some();
    let first: Vec<&Variant> = parts.parts.iter().filter_map(|part| part.first()).collect();
    let model_ambient_occlusion = match first.first() {
        Some(variant) => models.resolver().resolve(&variant.model).ambient_occlusion,
        None => false,
    };

    let fixed: Vec<&Variant> = match (varies, parts.source) {
        (true, PartSource::Multipart) => parts
            .parts
            .iter()
            .filter(|part| part.len() == 1)
            .filter_map(|part| part.first())
            .collect(),
        _ => first,
    };
    let baked = models.bake(&fixed)?;
    let own = geometry(set, state, &baked, sprites)?;

    let variants = if varies {
        let shapes = match parts.source {
            PartSource::Variants => {
                let mut shapes = Vec::new();
                for variant in parts.parts.iter().flatten().skip(1) {
                    let baked = models.bake(&[variant])?;
                    let other = geometry(set, state, &baked, sprites)?;
                    shapes.push(Shape {
                        cube: other.cube,
                        drop: other.drop,
                        quads: other.quads,
                    });
                }
                VariantShapes::Whole(shapes)
            }
            PartSource::Multipart => {
                let mut by_part = Vec::with_capacity(parts.parts.len());
                for part in &parts.parts {
                    if part.len() < 2 {
                        by_part.push(vec![Vec::new()]);
                        continue;
                    }
                    let mut by_variant = Vec::with_capacity(part.len());
                    for variant in part {
                        let baked = models.bake(&[variant])?;
                        by_variant.push(model_quads(
                            set,
                            state,
                            &baked,
                            sprites,
                            0..baked.quads.len(),
                        )?);
                    }
                    by_part.push(by_variant);
                }
                VariantShapes::Parts(by_part)
            }
        };
        Some(Box::new(StateVariants { pick, shapes }))
    } else {
        None
    };

    let mut quads = own.quads;
    let shapes = shape_quads(set, state, sprites)?;
    let shapes_only = baked.quads.is_empty() && variants.is_none() && !shapes.is_empty();
    quads.extend(shapes);

    Ok(BlockInfo {
        cube: own.cube,
        faces,
        drop: own.drop,
        quads,
        occludes: own.occludes,
        self_culls: own.self_culls,
        sturdy: own.sturdy,
        fluid,
        light,
        ambient_occlusion: model_ambient_occlusion && emission == 0 && !shapes_only,
        variants,
    })
}

/// The meshing geometry of a baked model: the cube faces that can merge with their
/// neighbours, the model quads and what the block covers.
struct Geometry {
    cube: Option<[CubeFace; 6]>,
    drop: u8,
    quads: Vec<ModelQuad>,
    occludes: bool,
    self_culls: bool,
    sturdy: u8,
}

fn geometry(
    set: &ResourceSet,
    state: &BlockStateKey,
    baked: &BakedBlock,
    sprites: &mut SpriteRegistry,
) -> Result<Geometry, BakeError> {
    let mut layers: Vec<u16> = Vec::with_capacity(baked.sprites.len());
    for sprite in &baked.sprites {
        layers.push(sprites.intern(set, sprite)?);
    }
    let sturdy = sturdy_faces(&baked.quads, &layers, sprites);

    let (mut cube_faces, mut extras) = split_cube(&baked.quads);
    let mut drop = 0;
    if cube_faces.is_none()
        && let Some((top, bottom, lowered)) = split_lowered(&baked.quads)
    {
        cube_faces = Some(std::array::from_fn(|dir| {
            if dir == Direction::Down as usize {
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
            let quad = baked.quads.get(index).ok_or(BakeError::PastQuads)?;
            let interned = *layers.get(quad.sprite).ok_or(BakeError::PastSprites)?;
            let pass = quad
                .pass
                .unwrap_or_else(|| Pass::from(sprites.opacity(interned)));
            worst = worst.max(pass);
            *face = CubeFace {
                sprite: interned,
                pass: pass as u8,
                tint: tint_of(state, quad.face.tint),
                emission: quad.face.light_emission,
                uv: UvOrientation::of_corners(&quad.face.uvs).unwrap_or_default(),
            };
        }
        cube = Some(built);
        occludes = drop == 0 && worst == Pass::Solid;
        self_culls = drop == 0 && worst == Pass::Translucent;
    }
    Ok(Geometry {
        cube,
        drop,
        quads: model_quads(set, state, baked, sprites, extras)?,
        occludes,
        self_culls,
        sturdy,
    })
}

fn model_quads(
    set: &ResourceSet,
    state: &BlockStateKey,
    baked: &BakedBlock,
    sprites: &mut SpriteRegistry,
    indices: impl IntoIterator<Item = usize>,
) -> Result<Vec<ModelQuad>, BakeError> {
    let mut quads = Vec::new();
    for index in indices {
        let quad = baked.quads.get(index).ok_or(BakeError::PastQuads)?;
        let sprite = baked
            .sprites
            .get(quad.sprite)
            .ok_or(BakeError::PastSprites)?;
        let interned = sprites.intern(set, sprite)?;
        quads.push(ModelQuad {
            positions: quad.face.positions,
            uvs: quad.face.uvs,
            cull: quad.face.cull,
            facing: quad.face.direction,
            face: face_group(&quad.face.positions),
            sprite: interned,
            pass: quad
                .pass
                .unwrap_or_else(|| Pass::from(sprites.opacity(interned))),
            shade: FaceShade::Direction(quad.face.shade_face.unwrap_or(quad.face.direction)),
            tint: tint_of(state, quad.face.tint),
            emission: quad.face.light_emission,
        });
    }
    Ok(quads)
}

/// The faces of the static block entity shapes of `state`. A neighbour never hides them.
fn shape_quads(
    pack: &ResourceSet,
    state: &BlockStateKey,
    sprites: &mut SpriteRegistry,
) -> Result<Vec<ModelQuad>, BakeError> {
    let shapes = pack
        .block_entity_models()
        .bake(&state.name, &state.pairs())?;
    let mut quads = Vec::with_capacity(shapes.len());
    for shape in shapes {
        quads.push(ModelQuad {
            positions: shape.positions,
            uvs: shape.uvs,
            cull: None,
            facing: shape.facing,
            face: face_group(&shape.positions),
            sprite: sprites.intern(pack, shape.texture)?,
            pass: surface_pass(shape.surface),
            shade: FaceShade::Factor(shape.shade),
            tint: shape.dye.map_or(Tint::None, Tint::dye),
            emission: 0,
        });
    }
    Ok(quads)
}

/// The terrain mesh has no blended pass for these faces yet, so `blend` draws as `cutout`.
fn surface_pass(surface: Surface) -> Pass {
    match surface {
        Surface::Opaque => Pass::Solid,
        Surface::Cutout | Surface::Blend => Pass::Cutout,
    }
}

const FACE_GRID: usize = 16;

/// The face occlusion shape of each side, on a sixteenth grid: the boxes that reach the
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

fn sturdy_faces(quads: &[BakedFace], layers: &[u16], sprites: &SpriteRegistry) -> u8 {
    let mut sides = [[0u16; FACE_GRID]; Direction::all().len()];
    for quad in quads {
        let Some(dir) = quad.face.cull else { continue };
        let Some(&layer) = layers.get(quad.sprite) else {
            continue;
        };
        if sprites.opacity(layer) != Opacity::Solid {
            continue;
        }
        if let Some(side) = sides.get_mut(dir as usize) {
            cover_face(side, &quad.face.positions, dir);
        }
    }
    let mut mask = 0;
    for (dir, side) in Direction::all().into_iter().zip(&sides) {
        if side.iter().all(|row| *row == u16::MAX) {
            mask |= 1 << dir as u8;
        }
    }
    mask
}

fn cover_face(side: &mut [u16; FACE_GRID], positions: &[Vec3; 4], dir: Direction) {
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
    if let Some(dir) = Direction::all()
        .into_iter()
        .find(|dir| normal.dot(dir.normal().as_vec3()) >= 1.0 - 1e-4)
    {
        return Some(dir as u8);
    }
    let diagonal = DIAGONALS
        .iter()
        .position(|&[x, z]| normal.dot(Vec3::new(x, 0.0, z).normalize()) >= 1.0 - 1e-4)?;
    Some(Direction::all().len() as u8 + diagonal as u8)
}

/// The colour each block multiplies into the faces that name a tint layer. A layer past the block's list, or a block with none, is left untinted.
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

fn split_cube(quads: &[BakedFace]) -> (Option<[usize; 6]>, Vec<usize>) {
    let mut faces = [usize::MAX; 6];
    let mut extras = Vec::new();
    for (index, quad) in quads.iter().enumerate() {
        match faces.get_mut(quad.face.direction as usize) {
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
fn split_lowered(quads: &[BakedFace]) -> Option<(usize, usize, u8)> {
    let bottom = quads
        .iter()
        .position(|quad| quad.face.direction == Direction::Down && is_cube_face(quad))?;
    quads.iter().enumerate().find_map(|(index, quad)| {
        if quad.face.direction != Direction::Up || quad.face.cull.is_some() || !cube_uvs(quad) {
            return None;
        }
        let height = quad.face.positions[0].y;
        let drop = (1.0 - height) * MODEL_STEPS;
        if !(1.0..MODEL_STEPS).contains(&drop.round()) || (drop - drop.round()).abs() > 1e-3 {
            return None;
        }
        let lowered = mcrs_minecraft_mesh::smooth_light::corners(
            Direction::Up,
            Vec3::ZERO,
            Vec3::new(1.0, height, 1.0),
        )
        .iter()
        .zip(&quad.face.positions)
        .all(|(at, position)| position.distance_squared(*at) <= 1e-6);
        lowered.then_some((index, bottom, drop.round() as u8))
    })
}

fn cube_uvs(quad: &BakedFace) -> bool {
    quad.face
        .uvs
        .iter()
        .zip(&CORNER_UV)
        .all(|(&uv, &want)| near_uv(uv, want))
}

fn is_cube_face(quad: &BakedFace) -> bool {
    if quad.face.cull != Some(quad.face.direction) {
        return false;
    }
    let on_cube = quad
        .face
        .positions
        .iter()
        .zip(cube_corners(quad.face.direction))
        .all(|(position, corner)| position.distance_squared(corner) <= 1e-6);
    on_cube && UvOrientation::of_corners(&quad.face.uvs).is_some()
}

fn near_uv([u, v]: [f32; 2], [want_u, want_v]: [f32; 2]) -> bool {
    (u - want_u).abs() <= 1e-4 && (v - want_v).abs() <= 1e-4
}

#[cfg(test)]
mod tests {
    #[test]
    fn every_state_drawn_in_code_bakes_with_its_entity_textures() {
        let corpus = crate::block_catalog::corpus();
        let mut sprites = SpriteRegistry::default();
        for name in [
            "chest",
            "trapped_chest",
            "ender_chest",
            "waxed_weathered_copper_chest",
            "shulker_box",
            "light_gray_shulker_box",
            "lime_banner",
            "black_wall_banner",
            "skeleton_skull",
            "wither_skeleton_wall_skull",
            "zombie_head",
            "player_wall_head",
            "creeper_head",
            "piglin_wall_head",
            "dragon_head",
            "decorated_pot",
            "enchanting_table",
            "lectern",
            "bell",
        ] {
            let block = corpus
                .block(&format!("minecraft:{name}"))
                .unwrap_or_else(|| panic!("the corpus has {name}"));
            for id in (0..block.state_count).filter_map(|offset| block.state_at(offset)) {
                let state = crate::block_catalog::state_key(corpus, id).unwrap();
                let info = super::build_one(
                    &mut BlockModels::new(ResourceSet::corpus()),
                    corpus.block_index(id).and_then(Block::from_id).unwrap(),
                    &state,
                    &PropertyTable::of_block(&block.properties),
                    corpus.state(id),
                    corpus.shape(corpus.state(id).occlusion_shape),
                    &mut sprites,
                )
                .unwrap_or_else(|reason| panic!("{} does not bake: {reason}", state.label()));
                assert!(
                    !info.quads.is_empty() || info.cube.is_some(),
                    "{} draws nothing",
                    state.label()
                );
            }
        }
    }

    #[test]
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

    #[test]
    fn ambient_occlusion_reads_its_neighbours_from_the_corpus() {
        let stone = bake_state("minecraft:stone", &[]);
        assert!(stone.light.full_collision && !stone.light.is_open());
        assert_eq!(stone.light.occlusion_shade, 0.2);

        let glass = bake_state("minecraft:glass", &[]);
        assert_eq!(
            glass.light.occlusion_shade, 1.0,
            "glass leaves the corners beside it lit"
        );
        assert!(glass.light.is_open(), "light passes through glass");

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
        assert!(!stairs.light.full_collision);

        assert!(
            !bake_state("minecraft:torch", &[]).ambient_occlusion,
            "a block that emits light is never occluded"
        );
    }

    #[test]
    fn a_block_lights_the_mesh_by_what_the_corpus_says_it_emits() {
        assert_eq!(bake_state("minecraft:torch", &[]).light.emission, 14);
        assert_eq!(bake_state("minecraft:glowstone", &[]).light.emission, 15);
        assert_eq!(bake_state("minecraft:stone", &[]).light.emission, 0);
    }

    use super::{
        Block, BlockModels, BlockStateKey, PropertyTable, ResourceSet, cube_corners, face_group,
        split_cube,
    };
    use crate::atlas::SpriteRegistry;
    use crate::bake::{self, BakedBlock};
    use bevy::math::Vec3;
    use mcrs_minecraft_core::Direction;
    use mcrs_minecraft_mesh::state_variants::VariantShapes;
    use mcrs_minecraft_mesh::uv_orientation::UvOrientation;

    fn baked(name: &str, props: &[(&str, &str)]) -> BakedBlock {
        let corpus = crate::block_catalog::corpus();
        let block = corpus.block(name).expect("the corpus has the block");
        BlockModels::new(ResourceSet::corpus())
            .bake_state(
                name,
                &PropertyTable::of_block(&block.properties),
                props,
                [0, 0, 0],
            )
            .unwrap_or_else(|reason| panic!("{name} does not bake: {reason}"))
    }

    fn bake_state(name: &str, props: &[(&str, &str)]) -> super::BlockInfo {
        let corpus = crate::block_catalog::corpus();
        let mut id = corpus.default_state(name).unwrap();
        for (key, value) in props {
            id = corpus
                .owner(id)
                .unwrap()
                .with_text(id, key, value)
                .unwrap_or_else(|| panic!("{name} has no {key}={value}"));
        }
        let state = crate::block_catalog::state_key(corpus, id).unwrap();
        super::build_one(
            &mut BlockModels::new(ResourceSet::corpus()),
            corpus
                .block_index(id)
                .and_then(Block::from_id)
                .expect("a corpus block is a registry entry"),
            &state,
            &PropertyTable::of_block(&corpus.owner(id).unwrap().properties),
            corpus.state(id),
            corpus.shape(corpus.state(id).occlusion_shape),
            &mut SpriteRegistry::default(),
        )
        .unwrap_or_else(|reason| panic!("{name} does not bake: {reason}"))
    }

    fn closed(name: &str, props: &[(&str, &str)]) -> Vec<&'static str> {
        let sturdy = bake_state(name, props).sturdy;
        Direction::all()
            .into_iter()
            .filter(|dir| sturdy >> *dir as u8 & 1 == 1)
            .map(|dir| dir.name())
            .collect()
    }

    #[test]
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
        for dir in Direction::all() {
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
            group >= Direction::all().len() as u8,
            "a pane must not be filed under an axis, it would vanish from one side"
        );
        let mirrored = [pane[3], pane[2], pane[1], pane[0]];
        assert_ne!(
            face_group(&mirrored),
            Some(group),
            "the same pane wound the other way faces the other way"
        );
    }

    #[test]
    fn a_face_hides_behind_a_neighbour_whose_shape_covers_it() {
        use mcrs_minecraft_mesh::block::face_hidden;
        let snow = |layers: &str| bake_state("minecraft:snow", &[("layers", layers)]);
        let (low, same, high) = (snow("2"), snow("2"), snow("5"));
        let (north, east, up, down) = (Direction::North as usize, Direction::East as usize, 1, 0);
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

    #[test]
    fn a_box_lower_than_its_cell_meshes_its_top_and_bottom_as_cube_faces() {
        for layers in 1..=7u8 {
            let snow = bake_state("minecraft:snow", &[("layers", &layers.to_string())]);
            assert!(snow.cube.is_some(), "{layers} layers of snow");
            assert_eq!(snow.drop, 32 - 4 * layers, "{layers} layers of snow");
            let baked = baked("minecraft:snow", &[("layers", &layers.to_string())]);
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

    #[test]
    fn a_turned_cube_still_tiles_and_a_weighted_block_varies_by_position() {
        let upright = baked("minecraft:oak_log", &[("axis", "y")]);
        assert!(split_cube(&upright.quads).0.is_some());

        let sideways = baked("minecraft:oak_log", &[("axis", "x")]);
        let (faces, _) = split_cube(&sideways.quads);
        let faces = faces.expect("a log turned on its side is still a cube");
        assert!(
            faces.iter().any(|&index| {
                UvOrientation::of_corners(&sideways.quads[index].face.uvs)
                    != Some(UvOrientation::IDENTITY)
            }),
            "a log turned on its side lays some sprite turned"
        );

        let grass = baked("minecraft:grass_block", &[("snowy", "false")]);
        let (cube, extras) = split_cube(&grass.quads);
        assert!(cube.is_some(), "the grass block hides a full cube");
        assert_eq!(extras.len(), 4, "four tinted side overlays are left over");

        let info = bake_state("minecraft:grass_block", &[("snowy", "false")]);
        let variants = info
            .variants
            .as_ref()
            .expect("the grass block has turned variants");
        let VariantShapes::Whole(shapes) = &variants.shapes else {
            panic!("the grass block has one part");
        };
        assert!(
            shapes.iter().all(|shape| shape.cube.is_some()),
            "each turn of the grass block is still a cube"
        );
    }
}
