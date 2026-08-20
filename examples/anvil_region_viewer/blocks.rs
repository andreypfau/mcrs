//! Bakes every block state the region references exactly once and sorts it into a geometry class.
//!
//! Baking touches the filesystem and parses JSON, so doing it per block would be hopeless on
//! millions of blocks. The region interns only a few hundred distinct states, so the whole catalog
//! is built up front and the mesher then works from plain arrays.

use bevy::math::{IVec3, Vec3};

use crate::anvil::{BlockStateKey, REGION_BLOCKS, SECTION_SIZE, World};
use crate::atlas::{Opacity, SpriteRef, SpriteRegistry};
use crate::pack::{MAX_SPRITES, MAX_SPRITE_ARRAYS};
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
    pub sprite: SpriteRef,
    pub pass: u8,
    pub tinted: bool,
}

/// One quad of a complex model, in block-local space with no lighting folded in yet.
#[derive(Clone)]
pub struct ModelQuad {
    pub positions: [Vec3; 4],
    pub uvs: [[f32; 2]; 4],
    pub cull: Option<Dir>,
    /// The face group this quad's geometry squarely points along, when it points along one: the
    /// six axes numbered as [`Dir`], then the four horizontal diagonals. Quads sharing a face
    /// group are all backfacing at once, so the culling pass can drop the whole run.
    pub face: Option<u8>,
    pub sprite: SpriteRef,
    pub pass: Pass,
    /// Vanilla's directional face shade, already applied per corner.
    pub shade: [u8; 4],
    pub tinted: bool,
}

#[derive(Clone)]
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
    /// Which of the six faces this block's own opaque geometry covers corner to corner, as a bit
    /// per [`Dir`]. Vanilla asks a voxel shape the same question, and the answer is what decides
    /// whether the water in a waterlogged block shows through its own stair or slab, and whether a
    /// fluid beside one is hidden by it. [`Self::occludes`] is the whole-block version and cannot
    /// stand in: a stair covers two of its faces and none of the other four.
    pub sturdy: u8,
    pub tint_kind: TintKind,
    pub emission: u8,
    /// The fluid filling this block, drawn by its own mesher rather than by either model path. A
    /// waterlogged block carries geometry *and* a fluid, so this sits beside the model rather than
    /// replacing it.
    pub fluid: Option<Fluid>,
}

impl Default for BlockInfo {
    fn default() -> Self {
        Self {
            cube: None,
            quads: Vec::new(),
            occludes: false,
            self_culls: false,
            sturdy: 0,
            tint_kind: TintKind::Grass,
            emission: 0,
            fluid: None,
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

/// Blocks vanilla gives a fluid to without a `waterlogged` property, because their own class
/// answers `getFluidState` with a water source.
const IMPLICITLY_WATERLOGGED: [&str; 5] = [
    "minecraft:bubble_column",
    "minecraft:kelp",
    "minecraft:kelp_plant",
    "minecraft:seagrass",
    "minecraft:tall_seagrass",
];

/// The fluid a block holds, as much of vanilla's `FluidState` as drawing one needs.
///
/// A fluid is not a model and never goes through the bakery: water and lava ship models with no
/// elements at all, and the surface of a fluid is a shape the client computes from the levels of
/// the eight blocks around it. What a block state carries here is only the input to that.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct Fluid {
    /// Lava draws opaque in the solid pass and takes no biome tint; water is blended and tinted.
    pub lava: bool,
    /// Vanilla's fluid amount, which is this block's own height in ninths and runs 1..=8. A block
    /// with the same fluid above it is a full cube whatever this says.
    pub amount: u8,
    /// The surface texture, used flat on the top and bottom of the fluid.
    pub still: SpriteRef,
    /// The flowing texture, used on every vertical face and on a surface that is moving.
    pub flow: SpriteRef,
    /// What a vertical face takes instead of the flowing texture where it meets something you can
    /// see through. Vanilla ships one for water and none for lava.
    pub overlay: Option<SpriteRef>,
}

/// Vanilla's `FlowingFluid.getLegacyLevel` read backwards: a block state's `level` is the amount
/// counted down from eight, with eight added again when the fluid is falling. Falling changes only
/// how fast the fluid spreads, never how it is drawn, so it is dropped here.
fn amount_of(level: u32) -> u8 {
    match level {
        0 => 8,
        1..=7 => 8 - level as u8,
        _ => (16u32.saturating_sub(level)).clamp(1, 8) as u8,
    }
}

/// The fluid a block state holds, and the two sprites drawing it needs.
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

/// A catalog with nothing baked into it yet. Regions are baked in as they arrive, because the
/// block states a world holds are only known once its files have been read.
pub fn empty() -> Catalog {
    Catalog {
        blocks: Vec::new(),
        sprites: SpriteRegistry::new(),
        // Slot zero is the untinted white every block that takes no biome colour points at.
        tints: vec![[1.0, 1.0, 1.0, 1.0]],
        failures: Vec::new(),
    }
}

/// Bakes whatever the world has interned since the last call.
///
/// Ids are handed out by appending, so an id already baked never changes meaning and a catalog
/// only ever grows. That is what lets a mesher hold an older, shorter catalog safely: it can only
/// name states that existed when it started.
pub fn extend(catalog: &mut Catalog, states: &[BlockStateKey], biomes: &[String]) {
    let neighbours = TinyWorld::default();
    for state in &states[catalog.blocks.len()..] {
        match build_one(state, &neighbours, &mut catalog.sprites) {
            Ok(info) => catalog.blocks.push(info),
            Err(reason) => {
                catalog.failures.push(format!("{}: {reason}", state.label()));
                catalog.blocks.push(BlockInfo::default());
            }
        }
    }
    // Past either of these the packed fields wrap and quads silently sample a different sprite,
    // which no validation layer anywhere would report.
    assert!(
        catalog.sprites.arrays().len() <= MAX_SPRITE_ARRAYS,
        "the pack uses {} sprite resolutions, but a packed quad can address only \
         {MAX_SPRITE_ARRAYS} arrays",
        catalog.sprites.arrays().len(),
    );
    // Animations take the top of the layer field and every array's still sprites take the bottom
    // of their own, so what has to fit is one array's stills beside every animation there is.
    for array in catalog.sprites.arrays() {
        assert!(
            array.stills() + catalog.sprites.animations().len() <= MAX_SPRITES,
            "{} still sprites are {}x{} and {} animations sit above them, but a quad can name \
             only {MAX_SPRITES} layers",
            array.stills(),
            array.size,
            array.size,
            catalog.sprites.animations().len(),
        );
    }

    extend_tints(catalog, biomes);
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
    let fluid = fluid_of(state, sprites)?;

    let baked = bake::bake(&state.name, &state.pairs(), IVec3::ZERO, world)?;
    if baked.quads.is_empty() {
        // Water and lava ship a model holding nothing but a particle texture, and block entities
        // (chests, beds, spawners) ship one with no elements at all; the client draws those with a
        // separate entity renderer this viewer does not have.
        return Ok(BlockInfo {
            fluid,
            ..BlockInfo::default()
        });
    }

    let mut layers: Vec<SpriteRef> = Vec::with_capacity(baked.sprites.len());
    for sprite in &baked.sprites {
        layers.push(sprites.intern(sprite)?);
    }

    // Taken before the cube is split off, because the six faces of a full cube are exactly the
    // ones that cover their own side and `split_cube` moves them out of the list.
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

/// How finely a face is measured for coverage. Model coordinates are already sixteenths of a
/// block, so this is exact for the boxes every block model is built out of.
const FACE_GRID: usize = 16;

/// Which sides of the unit cube the model closes off, as a bit per [`Dir`].
///
/// A side counts as closed when the opaque quads lying on it cover it outright, which has to be
/// asked of the quads together rather than one at a time: the closed side of a stair is its slab
/// and its step, two quads meeting halfway up, and either alone covers nothing. That is why the
/// coverage is painted onto a grid of sixteenths instead of tested as a rectangle — and painting a
/// stair, a slab and a shut trapdoor onto it comes out where vanilla's own occlusion shapes do,
/// because every block model there is is a union of boxes.
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

/// Paints the sixteenths of one side of the block that a quad lying flat on it covers.
///
/// A quad that only partly covers a sixteenth is not counted for it: a face has to be closed to
/// hide what is behind it, and half a texel of cover is a gap.
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
    // The two axes the face runs along, which are every axis but the one it faces.
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

/// The horizontal diagonals, in the order the culling pass numbers them after the six axes. These
/// are where the panes of a plant point, and they are the only direction off the axes that any
/// worthwhile number of quads share.
const DIAGONALS: [[f32; 2]; 4] = [[1.0, 1.0], [1.0, -1.0], [-1.0, 1.0], [-1.0, -1.0]];

/// The face group a quad squarely points along, or `None` when it points along none of them.
///
/// The tolerance has to be this tight, and `BakedQuad::dir` cannot stand in for the answer: that is
/// the nearest axis with no tolerance at all, so the two 45-degree panes of a plant come back
/// labelled `Up` and would disappear the moment the camera dropped below them.
///
/// Winding is counter-clockwise seen from outside, so the cross product of the first two edges is
/// the outward normal.
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
fn extend_tints(catalog: &mut Catalog, biomes: &[String]) {
    let done = (catalog.tints.len() - 1) / TINT_KINDS;
    if done == biomes.len() {
        return;
    }
    let grass_map = load_colormap("grass");
    let foliage_map = load_colormap("foliage");
    for name in &biomes[done..] {
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
        catalog.tints.extend_from_slice(&[grass, foliage, water]);
    }
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

/// Bakes one region file's biome colours into a `REGION_BLOCKS` square per tint kind, sampled in
/// the fragment shader by world x and z. Keeping the colour out of the vertex means the greedy
/// mesher can merge grass across a biome boundary, and linear filtering then blends the two
/// colours for free — which is also how the client avoids a hard seam down the middle of a chunk.
///
/// One file at a time because the map covering the whole window is written into as files land,
/// and each of them only knows its own square.
pub fn tint_square(world: &World, tints: &[[f32; 4]], corner: [usize; 2]) -> Vec<u8> {
    const SIZE: usize = REGION_BLOCKS;
    let mut out = vec![0u8; SIZE * SIZE * 4 * TINT_KINDS];
    for z in 0..SIZE {
        for x in 0..SIZE {
            let biome = surface_biome(world, corner[0] + x, corner[1] + z);
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
fn surface_biome(world: &World, x: usize, z: usize) -> u8 {
    let sx = x / SECTION_SIZE;
    let sz = z / SECTION_SIZE;
    let cell = ((z % SECTION_SIZE) / 4) * 4 + (x % SECTION_SIZE) / 4;
    for sy in (0..world.sections[1]).rev() {
        if world.section(sx, sy, sz).is_some() {
            return world.biome(sx, sy, sz, 3 * 16 + cell);
        }
    }
    0
}

#[cfg(test)]
mod tests {

    /// Vanilla stores a fluid's height in the block state as `getLegacyLevel` wrote it: the amount
    /// counted down from eight, with the whole scale repeated above eight for a falling fluid.
    /// Reading it back the wrong way round turns a trickle into a full block and a full block into
    /// a trickle, which no test of the mesher above it would ever notice.
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

    /// Bakes one block state the way the catalog does.
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

    /// The six sides a block closes off, named, so a failure says which side went missing rather
    /// than printing two numbers.
    fn closed(name: &str, props: &[(&str, &str)]) -> Vec<&'static str> {
        let sturdy = bake_state(name, props).sturdy;
        Dir::ALL
            .into_iter()
            .filter(|dir| sturdy >> *dir as u8 & 1 == 1)
            .map(|dir| dir.name())
            .collect()
    }

    /// Which sides of a block are closed is what decides whether the water in a waterlogged block
    /// shows through its own geometry, and it cannot be read off one quad at a time: the closed
    /// side of a stair is a slab and a step meeting halfway up it, and neither covers the side
    /// alone. Vanilla asks a voxel shape the same question and gets these same answers.
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
        // A stair's slab covers the lower half of all four sides and its step covers the upper half
        // of the one it stands against, so exactly that one side comes out closed.
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

    /// A quad joins a face group only when it points along that direction squarely, and the
    /// nearest axis is not good enough to decide it: the two diagonal panes of a plant are nearest
    /// to some axis like everything else, and grouping them by it would cull them away from one
    /// side. They get the diagonal they really point along instead, which is what lets the culling
    /// pass drop the half of them that faces away.
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

        // A grass block is a cube plus four side overlays: the cube must still be found, so the
        // block occludes its neighbours and its bulk stays on the greedy path.
        let grass = bake::bake("minecraft:grass_block", &[("snowy", "false")], IVec3::ZERO, &world)
            .expect("grass block bakes");
        let (cube, extras) = split_cube(&grass.quads);
        assert!(cube.is_some(), "the grass block hides a full cube");
        assert_eq!(extras.len(), 4, "four tinted side overlays are left over");
    }
}
