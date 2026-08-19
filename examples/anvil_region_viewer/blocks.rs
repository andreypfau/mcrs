//! Bakes every block state the region references exactly once and sorts it into a geometry class.
//!
//! Baking touches the filesystem and parses JSON, so doing it per block would be hopeless on
//! millions of blocks. The region interns only a few hundred distinct states, so the whole catalog
//! is built up front and the mesher then works from plain arrays.

use std::collections::HashMap;

use bevy::math::{IVec3, Vec3};

use crate::anvil::{BlockStateKey, REGION_CHUNKS, Region, SECTION_SIZE};
use crate::atlas::{Opacity, SpriteRegistry};
use crate::pack::MAX_SPRITES;
use crate::bake::{self, Dir, TinyWorld};
use crate::model;

/// Which draw pass a quad belongs to. Ordered so `max` picks the more permissive one.
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum Pass {
    Solid = 0,
    Cutout = 1,
    Translucent = 2,
}

impl Pass {
    pub const COUNT: usize = 3;

    fn of(opacity: Opacity) -> Pass {
        match opacity {
            Opacity::Solid => Pass::Solid,
            Opacity::Cutout => Pass::Cutout,
            Opacity::Translucent => Pass::Translucent,
        }
    }
}

/// Which colour table a tinted face samples. Vanilla picks this per block in code, not from the
/// resource pack, so the block name is the only signal available here.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum TintKind {
    Grass = 0,
    Foliage = 1,
    Water = 2,
}

pub const TINT_KINDS: usize = 3;

#[derive(Copy, Clone, Default)]
pub struct CubeFace {
    pub layer: u16,
    pub pass: u8,
    pub tinted: bool,
}

/// One quad of a complex model, in block-local space with no lighting folded in yet.
#[derive(Clone)]
pub struct ModelQuad {
    pub positions: [Vec3; 4],
    pub uvs: [[f32; 2]; 4],
    pub cull: Option<Dir>,
    /// The axis this quad's geometry squarely faces, when it has one. Quads sharing a face group
    /// are all backfacing at once, so the culling pass can drop the whole run.
    pub face: Option<Dir>,
    pub layer: u16,
    pub pass: Pass,
    /// Vanilla's directional face shade, already applied per corner.
    pub shade: [u8; 4],
    pub tinted: bool,
}

pub struct BlockInfo {
    /// The six identity-UV faces of the unit cube, when the model contains them. This is the greedy
    /// mesher's input, and a model may have these *and* extra geometry: a grass block is a cube
    /// plus four tinted side overlays.
    pub cube: Option<[CubeFace; 6]>,
    /// Whatever the greedy mesher cannot take, baked quad by quad.
    pub quads: Vec<ModelQuad>,
    /// Hides the touching face of any neighbour: the model covers the block in opaque texels.
    /// Deliberately independent of greedy-meshability — a grass block occludes its neighbours even
    /// though its overlay keeps it off the fast path.
    pub occludes: bool,
    /// Hides the touching face of an identical neighbour, the way glass and water do.
    pub self_culls: bool,
    pub tint_kind: TintKind,
    pub emission: u8,
}

impl Default for BlockInfo {
    fn default() -> Self {
        Self {
            cube: None,
            quads: Vec::new(),
            occludes: false,
            self_culls: false,
            tint_kind: TintKind::Grass,
            emission: 0,
        }
    }
}

pub struct Catalog {
    pub blocks: Vec<BlockInfo>,
    pub sprites: SpriteRegistry,
    /// Slot 0 is untinted white; slot `1 + biome * TINT_KINDS + kind` is that biome's colour.
    pub tints: Vec<[f32; 4]>,
    /// Block states that could not be baked, with the reason. Reported once at startup.
    pub failures: Vec<String>,
}

/// Per face: the normal axis, whether the face sits on the positive side of the block, and the two
/// world axes the sprite's `u` and `v` run along with their direction. Read off vanilla's own bake
/// of `block/cube_all`; `cube_uv_matches_the_vanilla_bake` pins it.
pub const FACE_AXES: [[u8; 6]; 6] = [
    // n_axis, n_positive, u_axis, u_positive, v_axis, v_positive
    [1, 0, 0, 1, 2, 0], // Down
    [1, 1, 0, 1, 2, 1], // Up
    [2, 0, 0, 0, 1, 0], // North
    [2, 1, 0, 1, 1, 0], // South
    [0, 0, 2, 1, 1, 0], // West
    [0, 1, 2, 0, 1, 0], // East
];

/// The four corners of a quad, in the order vanilla winds them, as `(u, v)` in 0..1.
pub const CORNER_UV: [[f32; 2]; 4] = [[0.0, 0.0], [0.0, 1.0], [1.0, 1.0], [1.0, 0.0]];

/// Corner position of a full-block face, in block-local space.
pub fn cube_corner(dir: Dir, corner: usize) -> Vec3 {
    let a = FACE_AXES[dir as usize];
    let (cu, cv) = (CORNER_UV[corner][0], CORNER_UV[corner][1]);
    let mut p = [0.0f32; 3];
    p[a[0] as usize] = if a[1] == 1 { 1.0 } else { 0.0 };
    p[a[2] as usize] = if a[3] == 1 { cu } else { 1.0 - cu };
    p[a[4] as usize] = if a[5] == 1 { cv } else { 1.0 - cv };
    Vec3::from(p)
}

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

/// Blocks whose model carries no geometry because the client renders them as a fluid. They are
/// meshed as plain cubes of their still texture.
///
/// ponytail: a real fluid mesher would slope the surface by `level` and use the flowing texture.
/// A viewer of a mostly-underground region does not miss it; add it when surface oceans matter.
const FLUIDS: [(&str, &str); 3] = [
    ("minecraft:water", "minecraft:block/water_still"),
    ("minecraft:bubble_column", "minecraft:block/water_still"),
    ("minecraft:lava", "minecraft:block/lava_still"),
];

pub fn build(region: &Region) -> Catalog {
    let mut sprites = SpriteRegistry::new();
    let mut blocks: Vec<BlockInfo> = Vec::with_capacity(region.states.len());
    let mut failures = Vec::new();
    let world = TinyWorld::default();

    for state in &region.states {
        match build_one(state, &world, &mut sprites) {
            Ok(info) => blocks.push(info),
            Err(reason) => {
                failures.push(format!("{}: {reason}", state.label()));
                blocks.push(BlockInfo::default());
            }
        }
    }
    sprites.finish();
    // Past this the packed layer field wraps and quads silently sample a different sprite, which
    // no validation layer anywhere would report.
    assert!(
        sprites.len() <= MAX_SPRITES,
        "the region references {} sprites, but a packed quad can address only {MAX_SPRITES}",
        sprites.len(),
    );

    Catalog {
        blocks,
        sprites,
        tints: build_tints(region),
        failures,
    }
}

fn build_one(
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

    if let Some((_, sprite)) = FLUIDS.iter().find(|(name, _)| *name == state.name) {
        let layer = sprites.intern(sprite)?;
        let pass = Pass::of(sprites.sprites[layer as usize].opacity);
        let tinted = state.name != "minecraft:lava";
        return Ok(BlockInfo {
            cube: Some([CubeFace {
                layer,
                pass: pass as u8,
                tinted,
            }; 6]),
            quads: Vec::new(),
            occludes: false,
            self_culls: true,
            tint_kind,
            emission,
        });
    }

    let baked = bake::bake(&state.name, &state.pairs(), IVec3::ZERO, world)?;
    if baked.quads.is_empty() {
        // Block entities (chests, beds, spawners) ship a model with no elements; the client draws
        // them with a separate entity renderer this viewer does not have.
        return Ok(BlockInfo::default());
    }

    let mut layers: Vec<u16> = Vec::with_capacity(baked.sprites.len());
    for sprite in &baked.sprites {
        layers.push(sprites.intern(sprite)?);
    }

    let (cube_faces, extras) = split_cube(&baked.quads);
    let mut cube = None;
    let mut occludes = false;
    let mut self_culls = false;
    if let Some(faces) = cube_faces {
        let mut built = [CubeFace::default(); 6];
        let mut worst = Pass::Solid;
        for dir in Dir::ALL {
            let quad = &baked.quads[faces[dir as usize]];
            let layer = layers[quad.sprite];
            let pass = Pass::of(sprites.sprites[layer as usize].opacity);
            worst = worst.max(pass);
            built[dir as usize] = CubeFace {
                layer,
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
        let layer = layers[quad.sprite];
        quads.push(ModelQuad {
            positions: quad.positions,
            uvs: quad.uvs,
            cull: quad.cull,
            face: face_group(&quad.positions),
            layer,
            pass: Pass::of(sprites.sprites[layer as usize].opacity),
            shade: quad.color,
            tinted: quad.tint.is_some(),
        });
    }

    Ok(BlockInfo {
        cube,
        quads,
        occludes,
        self_culls,
        tint_kind,
        emission,
    })
}

/// The axis a quad squarely faces, or `None` when it faces none of them squarely.
///
/// The tolerance has to be this tight, and `BakedQuad::dir` cannot stand in for the answer: that is
/// the nearest axis with no tolerance at all, so the two 45-degree panes of a plant come back
/// labelled `Up` and would disappear the moment the camera dropped below them.
///
/// Winding is counter-clockwise seen from outside, so the cross product of the first two edges is
/// the outward normal.
fn face_group(positions: &[Vec3; 4]) -> Option<Dir> {
    let normal = (positions[1] - positions[0])
        .cross(positions[2] - positions[0])
        .normalize_or_zero();
    Dir::ALL
        .into_iter()
        .find(|dir| normal.dot(dir.normal()) >= 1.0 - 1e-4)
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

/// Splits a baked model into the six faces of the unit cube — the part the greedy mesher can
/// merge — and everything left over.
///
/// A face qualifies only if it sits exactly on the cube boundary, is culled by its own direction
/// and carries vanilla's standard UVs, since the greedy quad reconstructs its UVs from `w` and `h`
/// and a rotated or inset mapping would not tile. The first match per direction wins, which for a
/// grass block picks the opaque dirt-and-grass cube and leaves the overlay to the complex path,
/// drawn on top of it afterwards.
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
        // Not a cube after all; every quad belongs to the complex path.
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

#[derive(serde::Deserialize)]
struct BiomeFile {
    #[serde(default)]
    temperature: f32,
    #[serde(default)]
    downfall: f32,
    #[serde(default)]
    effects: BiomeEffects,
}

#[derive(serde::Deserialize, Default)]
struct BiomeEffects {
    #[serde(default)]
    water_color: Option<u32>,
    #[serde(default)]
    grass_color: Option<u32>,
    #[serde(default)]
    foliage_color: Option<u32>,
}

/// Vanilla samples grass and foliage colour from a 256×256 colourmap indexed by the biome's
/// temperature and downfall; the per-biome `effects` overrides win when present.
fn build_tints(region: &Region) -> Vec<[f32; 4]> {
    let grass_map = load_colormap("grass");
    let foliage_map = load_colormap("foliage");
    let mut tints = Vec::with_capacity(1 + region.biomes.len() * TINT_KINDS);
    tints.push([1.0, 1.0, 1.0, 1.0]);

    let mut cache: HashMap<String, [[f32; 4]; TINT_KINDS]> = HashMap::new();
    for name in &region.biomes {
        let colors = cache.entry(name.clone()).or_insert_with(|| {
            let file = load_biome(name);
            let (temperature, downfall, effects) = match file {
                Some(file) => (file.temperature, file.downfall, file.effects),
                None => (0.5, 0.5, BiomeEffects::default()),
            };
            let grass = effects
                .grass_color
                .map(rgb)
                .or_else(|| sample_colormap(&grass_map, temperature, downfall))
                .unwrap_or([0.56, 0.73, 0.35, 1.0]);
            let foliage = effects
                .foliage_color
                .map(rgb)
                .or_else(|| sample_colormap(&foliage_map, temperature, downfall))
                .unwrap_or([0.29, 0.60, 0.21, 1.0]);
            let water = effects.water_color.map(rgb).unwrap_or([0.25, 0.46, 0.89, 1.0]);
            [grass, foliage, water]
        });
        tints.extend_from_slice(colors);
    }
    tints
}

fn load_biome(name: &str) -> Option<BiomeFile> {
    let path = model::resource_path(name, "worldgen/biome", "json");
    let text = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&text).ok()
}

fn load_colormap(name: &str) -> Option<Vec<u8>> {
    use bevy::asset::RenderAssetUsages;
    use bevy::image::{CompressedImageFormats, ImageSampler, ImageType};
    use bevy::prelude::Image;

    let path = model::resource_path(
        &format!("minecraft:colormap/{name}"),
        "textures",
        "png",
    );
    let bytes = std::fs::read(path).ok()?;
    let image = Image::from_buffer(
        &bytes,
        ImageType::Extension("png"),
        CompressedImageFormats::NONE,
        false,
        ImageSampler::nearest(),
        RenderAssetUsages::default(),
    )
    .ok()?;
    if image.width() != 256 || image.height() != 256 {
        return None;
    }
    image.data
}

fn sample_colormap(map: &Option<Vec<u8>>, temperature: f32, downfall: f32) -> Option<[f32; 4]> {
    let map = map.as_ref()?;
    let t = temperature.clamp(0.0, 1.0);
    let d = downfall.clamp(0.0, 1.0) * t;
    let column = ((1.0 - t) * 255.0) as usize;
    let row = ((1.0 - d) * 255.0) as usize;
    let offset = (row * 256 + column) * 4;
    if offset + 3 >= map.len() {
        return None;
    }
    Some([
        map[offset] as f32 / 255.0,
        map[offset + 1] as f32 / 255.0,
        map[offset + 2] as f32 / 255.0,
        1.0,
    ])
}

fn rgb(packed: u32) -> [f32; 4] {
    [
        ((packed >> 16) & 0xff) as f32 / 255.0,
        ((packed >> 8) & 0xff) as f32 / 255.0,
        (packed & 0xff) as f32 / 255.0,
        1.0,
    ]
}

/// Bakes the region's biome colours into a 512×512 map per tint kind, sampled in the fragment
/// shader by world x and z. Keeping the colour out of the vertex means the greedy mesher can merge
/// grass across a biome boundary, and linear filtering then blends the two colours for free —
/// which is also how the client avoids a hard seam down the middle of a chunk.
pub fn tint_map(region: &Region, tints: &[[f32; 4]]) -> Vec<u8> {
    const SIZE: usize = REGION_CHUNKS * SECTION_SIZE;
    let mut out = vec![0u8; SIZE * SIZE * 4 * TINT_KINDS];
    for z in 0..SIZE {
        for x in 0..SIZE {
            let biome = surface_biome(region, x, z);
            for kind in 0..TINT_KINDS {
                let slot = 1 + biome as usize * TINT_KINDS + kind;
                let color = tints.get(slot).copied().unwrap_or([1.0; 4]);
                let offset = (kind * SIZE * SIZE + z * SIZE + x) * 4;
                for channel in 0..4 {
                    out[offset + channel] = (color[channel].clamp(0.0, 1.0) * 255.0) as u8;
                }
            }
        }
    }
    out
}

/// The highest section that exists over this column decides the colour: tinted blocks are grass,
/// foliage and water, which all sit at or near the surface.
fn surface_biome(region: &Region, x: usize, z: usize) -> u8 {
    let cx = x / SECTION_SIZE;
    let cz = z / SECTION_SIZE;
    let cell = ((z % SECTION_SIZE) / 4) * 4 + (x % SECTION_SIZE) / 4;
    for sy in (0..region.sections_y).rev() {
        if let Some(section) = region.section(cx, sy, cz) {
            return section.biomes[3 * 16 + cell];
        }
    }
    0
}

#[cfg(test)]
mod tests {
    use super::{cube_corner, face_group, split_cube};
    use crate::bake::{self, Dir, TinyWorld};
    use bevy::math::{IVec3, Vec3};

    /// A quad joins a face group only when it faces that axis squarely, and the nearest axis is not
    /// good enough to decide it: the two diagonal panes of a plant are nearest to some axis like
    /// everything else, and grouping them by it would cull them away from one side.
    #[test]
    fn only_a_squarely_facing_quad_joins_a_face_group() {
        for dir in Dir::ALL {
            let face = std::array::from_fn(|corner| cube_corner(dir, corner));
            assert_eq!(face_group(&face), Some(dir), "the {} face of a cube", dir.name());
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
        assert_eq!(face_group(&pane), None, "a plant's diagonal pane");
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

        // A grass block is a cube plus four side overlays: the cube must still be found, so the
        // block occludes its neighbours and its bulk stays on the greedy path.
        let grass = bake::bake("minecraft:grass_block", &[("snowy", "false")], IVec3::ZERO, &world)
            .expect("grass block bakes");
        let (cube, extras) = split_cube(&grass.quads);
        assert!(cube.is_some(), "the grass block hides a full cube");
        assert_eq!(extras.len(), 4, "four tinted side overlays are left over");
    }
}
