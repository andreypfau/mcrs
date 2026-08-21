use bevy::math::{IVec3, Vec3};

use crate::anvil::BlockStateKey;
use crate::atlas::{Opacity, SpriteRef, SpriteRegistry};
use crate::bake::{self, Dir, TinyWorld};

use super::{BlockInfo, CORNER_UV, CubeFace, FACE_AXES, ModelQuad, Pass, TintKind, cube_corner};

const EMISSION: [(&str, u8); 9] = [
    ("minecraft:lava", 15),
    ("minecraft:sea_lantern", 15),
    ("minecraft:crying_obsidian", 10),
    ("minecraft:magma_block", 3),
    ("minecraft:amethyst_cluster", 5),
    ("minecraft:large_amethyst_bud", 4),
    ("minecraft:medium_amethyst_bud", 2),
    ("minecraft:small_amethyst_bud", 1),
    ("minecraft:budding_amethyst", 0),
];

const IMPLICITLY_WATERLOGGED: [&str; 5] = [
    "minecraft:bubble_column",
    "minecraft:kelp",
    "minecraft:kelp_plant",
    "minecraft:seagrass",
    "minecraft:tall_seagrass",
];

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct Fluid {
    pub lava: bool,
    pub amount: u8,
    pub still: SpriteRef,
    pub flow: SpriteRef,
    pub overlay: Option<SpriteRef>,
}

fn amount_of(level: u32) -> u8 {
    match level {
        0 => 8,
        1..=7 => 8 - level as u8,
        _ => (16u32.saturating_sub(level)).clamp(1, 8) as u8,
    }
}

fn fluid_of(
    state: &BlockStateKey,
    sprites: &mut SpriteRegistry,
) -> Result<Option<Fluid>, String> {
    let prop = |key: &str| {
        state
            .props
            .iter()
            .find(|(name, _)| name == key)
            .map(|(_, value)| value.as_str())
    };
    let (lava, amount) = match state.name.as_str() {
        "minecraft:water" | "minecraft:lava" => {
            let level = prop("level").and_then(|value| value.parse().ok()).unwrap_or(0);
            (state.name == "minecraft:lava", amount_of(level))
        }
        name if IMPLICITLY_WATERLOGGED.contains(&name) => (false, 8),
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
        false => Some(sprites.intern("minecraft:block/water_overlay")?),
    };
    Ok(Some(Fluid {
        lava,
        amount,
        still: sprites.intern(still)?,
        flow: sprites.intern(flow)?,
        overlay,
    }))
}

pub(super) fn build_one(
    state: &BlockStateKey,
    world: &TinyWorld,
    sprites: &mut SpriteRegistry,
) -> Result<BlockInfo, String> {
    if state.name == "minecraft:air"
        || state.name == "minecraft:cave_air"
        || state.name == "minecraft:void_air"
    {
        return Ok(BlockInfo::default());
    }

    let emission = EMISSION
        .iter()
        .find(|(name, _)| *name == state.name)
        .map(|(_, level)| *level)
        .unwrap_or(0);
    let tint_kind = tint_kind_of(&state.name);
    let fluid = fluid_of(state, sprites)?;

    let baked = bake::bake(&state.name, &state.pairs(), IVec3::ZERO, world)?;
    if baked.quads.is_empty() {
        return Ok(BlockInfo {
            fluid,
            ..BlockInfo::default()
        });
    }

    let mut layers: Vec<SpriteRef> = Vec::with_capacity(baked.sprites.len());
    for sprite in &baked.sprites {
        layers.push(sprites.intern(sprite)?);
    }

    let sturdy = sturdy_faces(&baked.quads, &layers, sprites);

    let (cube_faces, extras) = split_cube(&baked.quads);
    let mut cube = None;
    let mut occludes = false;
    let mut self_culls = false;
    if let Some(faces) = cube_faces {
        let mut built = [CubeFace::default(); 6];
        let mut worst = Pass::Solid;
        for dir in Dir::ALL {
            let quad = &baked.quads[faces[dir as usize]];
            let interned = layers[quad.sprite];
            let pass = Pass::of(sprites.opacity(interned));
            worst = worst.max(pass);
            built[dir as usize] = CubeFace {
                sprite: interned,
                pass: pass as u8,
                tinted: quad.tint.is_some(),
            };
        }
        cube = Some(built);
        occludes = worst == Pass::Solid;
        self_culls = worst == Pass::Translucent;
    }

    let mut quads = Vec::with_capacity(extras.len());
    for &index in &extras {
        let quad = &baked.quads[index];
        let interned = layers[quad.sprite];
        quads.push(ModelQuad {
            positions: quad.positions,
            uvs: quad.uvs,
            cull: quad.cull,
            face: face_group(&quad.positions),
            sprite: interned,
            pass: Pass::of(sprites.opacity(interned)),
            shade: quad.color,
            tinted: quad.tint.is_some(),
        });
    }

    Ok(BlockInfo {
        cube,
        quads,
        occludes,
        self_culls,
        sturdy,
        tint_kind,
        emission,
        fluid,
    })
}

const FACE_GRID: usize = 16;

fn sturdy_faces(
    quads: &[bake::BakedQuad],
    layers: &[SpriteRef],
    sprites: &SpriteRegistry,
) -> u8 {
    let mut sides = [[0u16; FACE_GRID]; Dir::ALL.len()];
    for quad in quads {
        let Some(dir) = quad.cull else { continue };
        if sprites.opacity(layers[quad.sprite]) != Opacity::Solid {
            continue;
        }
        cover_face(&mut sides[dir as usize], &quad.positions, dir);
    }
    let mut mask = 0;
    for dir in Dir::ALL {
        if sides[dir as usize].iter().all(|row| *row == u16::MAX) {
            mask |= 1 << dir as u8;
        }
    }
    mask
}

fn cover_face(side: &mut [u16; FACE_GRID], positions: &[Vec3; 4], dir: Dir) {
    let axes = FACE_AXES[dir as usize];
    let normal = axes[0] as usize;
    let plane = if axes[1] == 1 { 1.0 } else { 0.0 };
    let mut low = [f32::INFINITY; 3];
    let mut high = [f32::NEG_INFINITY; 3];
    for position in positions {
        let point = [position.x, position.y, position.z];
        if (point[normal] - plane).abs() > 1e-4 {
            return;
        }
        for axis in 0..3 {
            low[axis] = low[axis].min(point[axis]);
            high[axis] = high[axis].max(point[axis]);
        }
    }
    let steps = FACE_GRID as f32;
    let cell = |value: f32, round_up: bool| {
        let scaled = value * steps;
        let rounded = if round_up { scaled.ceil() } else { scaled.floor() };
        rounded.clamp(0.0, steps) as usize
    };
    let mut tangents = (0..3).filter(|axis| *axis != normal);
    let (rows, columns) = (tangents.next().unwrap(), tangents.next().unwrap());
    let span = cell(low[columns], true)..cell(high[columns], false);
    if span.is_empty() {
        return;
    }
    let bits = (((1u32 << span.len()) - 1) << span.start) as u16;
    for row in cell(low[rows], true)..cell(high[rows], false) {
        side[row] |= bits;
    }
}

const DIAGONALS: [[f32; 2]; 4] = [[1.0, 1.0], [1.0, -1.0], [-1.0, 1.0], [-1.0, -1.0]];

fn face_group(positions: &[Vec3; 4]) -> Option<u8> {
    let normal = (positions[1] - positions[0])
        .cross(positions[2] - positions[0])
        .normalize_or_zero();
    if let Some(dir) = Dir::ALL
        .into_iter()
        .find(|dir| normal.dot(dir.normal()) >= 1.0 - 1e-4)
    {
        return Some(dir as u8);
    }
    let diagonal = DIAGONALS.iter().position(|&[x, z]| {
        normal.dot(Vec3::new(x, 0.0, z).normalize()) >= 1.0 - 1e-4
    })?;
    Some(Dir::ALL.len() as u8 + diagonal as u8)
}

fn tint_kind_of(name: &str) -> TintKind {
    if name.ends_with("_leaves") || name.ends_with("vine") || name == "minecraft:glow_lichen" {
        return TintKind::Foliage;
    }
    if name == "minecraft:water" || name == "minecraft:bubble_column" {
        return TintKind::Water;
    }
    TintKind::Grass
}

fn split_cube(quads: &[bake::BakedQuad]) -> (Option<[usize; 6]>, Vec<usize>) {
    let mut faces = [usize::MAX; 6];
    let mut extras = Vec::new();
    for (index, quad) in quads.iter().enumerate() {
        let slot = faces[quad.dir as usize];
        if slot == usize::MAX && is_cube_face(quad) {
            faces[quad.dir as usize] = index;
        } else {
            extras.push(index);
        }
    }
    if faces.contains(&usize::MAX) {
        return (None, (0..quads.len()).collect());
    }
    (Some(faces), extras)
}

fn is_cube_face(quad: &bake::BakedQuad) -> bool {
    if quad.cull != Some(quad.dir) {
        return false;
    }
    for corner in 0..4 {
        if quad.positions[corner].distance_squared(cube_corner(quad.dir, corner)) > 1e-6 {
            return false;
        }
        if (quad.uvs[corner][0] - CORNER_UV[corner][0]).abs() > 1e-4
            || (quad.uvs[corner][1] - CORNER_UV[corner][1]).abs() > 1e-4
        {
            return false;
        }
    }
    true
}


#[cfg(test)]
mod tests {

    #[test]
    fn a_fluid_level_reads_back_as_the_height_vanilla_gives_it() {
        let ninths = [8u8, 7, 6, 5, 4, 3, 2, 1, 8, 7, 6, 5, 4, 3, 2, 1];
        for (level, height) in ninths.iter().enumerate() {
            assert_eq!(super::amount_of(level as u32), *height, "level {level}");
        }
    }

    use super::{BlockStateKey, cube_corner, face_group, split_cube};
    use crate::atlas::SpriteRegistry;
    use crate::bake::{self, Dir, TinyWorld};
    use bevy::math::{IVec3, Vec3};

    fn bake_state(name: &str, props: &[(&str, &str)]) -> super::BlockInfo {
        let state = BlockStateKey {
            name: name.to_string(),
            props: props
                .iter()
                .map(|(key, value)| (key.to_string(), value.to_string()))
                .collect(),
        };
        super::build_one(&state, &TinyWorld::default(), &mut SpriteRegistry::new())
            .unwrap_or_else(|reason| panic!("{name} does not bake: {reason}"))
    }

    fn closed(name: &str, props: &[(&str, &str)]) -> Vec<&'static str> {
        let sturdy = bake_state(name, props).sturdy;
        Dir::ALL
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
                &[("facing", "east"), ("half", "bottom"), ("shape", "straight")],
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
            closed("minecraft:oak_leaves", &[("persistent", "false"), ("distance", "7")])
                .is_empty(),
            "leaves are a cube full of holes and hide nothing"
        );
    }

    #[test]
    fn a_quad_joins_the_face_group_it_squarely_points_along() {
        for dir in Dir::ALL {
            let face = std::array::from_fn(|corner| cube_corner(dir, corner));
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
            Dir::nearest(normal).is_some(),
            "the nearest axis answers even for a pane that faces none of them"
        );
        let group = face_group(&pane).expect("a plant's pane points along a diagonal");
        assert!(
            group >= Dir::ALL.len() as u8,
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
    fn cube_uv_matches_the_vanilla_bake() {
        let baked = bake::bake("minecraft:stone", &[], IVec3::ZERO, &TinyWorld::default())
            .expect("stone bakes");
        let (faces, extras) = split_cube(&baked.quads);
        let faces = faces.expect("stone is a full cube");
        assert!(extras.is_empty(), "stone has nothing beyond its cube");
        for dir in Dir::ALL {
            let quad = &baked.quads[faces[dir as usize]];
            for corner in 0..4 {
                assert_eq!(
                    quad.positions[corner],
                    cube_corner(dir, corner),
                    "{dir:?} corner {corner}"
                );
            }
        }
    }

    #[test]
    fn a_rotated_log_is_not_greedy_meshable() {
        let world = TinyWorld::default();
        let upright = bake::bake("minecraft:oak_log", &[("axis", "y")], IVec3::ZERO, &world)
            .expect("upright log bakes");
        assert!(split_cube(&upright.quads).0.is_some());

        let sideways = bake::bake("minecraft:oak_log", &[("axis", "x")], IVec3::ZERO, &world)
            .expect("sideways log bakes");
        assert!(
            split_cube(&sideways.quads).0.is_none(),
            "a log turned on its side has rotated UVs and cannot tile"
        );

        let grass = bake::bake("minecraft:grass_block", &[("snowy", "false")], IVec3::ZERO, &world)
            .expect("grass block bakes");
        let (cube, extras) = split_cube(&grass.quads);
        assert!(cube.is_some(), "the grass block hides a full cube");
        assert_eq!(extras.len(), 4, "four tinted side overlays are left over");
    }
}
